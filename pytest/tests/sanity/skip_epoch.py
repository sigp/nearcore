# Tests a situation when in a given shard has all BPs offline
# Two specific cases:
#  - BPs never showed up to begin with, since genesis
#  - BPs went offline after some epoch
# Warn: this test may not clean up ~/.near if fails early

import sys, time, base58

sys.path.append('lib')

from cluster import init_cluster, spin_up_node, load_config
from transaction import sign_staking_tx
from utils import TxContext

TIMEOUT = 600
TWENTY_FIVE = 25

config = load_config()
near_root, node_dirs = init_cluster(2, 1, 2, config, [["min_gas_price", 0], ["max_inflation_rate", 0], ["epoch_length", 7], ["block_producer_kickout_threshold", 80]], {2: {"tracked_shards": [0, 1]}})

started = time.time()

boot_node = spin_up_node(config, near_root, node_dirs[0], 0, None, None)
#node1 = spin_up_node(config, near_root, node_dirs[1], 1, boot_node.node_key.pk, boot_node.addr())
observer = spin_up_node(config, near_root, node_dirs[2], 2, boot_node.node_key.pk, boot_node.addr())

# It takes a while for test2 account to appear
ctx = TxContext([0, 0, 0], [boot_node, None, observer])
print('ha')
initial_balances = ctx.get_balances()
total_supply = sum(initial_balances)

print("Initial balances: %s\nTotal supply: %s" % (initial_balances, total_supply))

seen_boot_heights = set()
sent_txs = False
largest_height = 0

# 1. Make the first node get to height 25. The second epoch will end around height 15-16,
#    which would already result in a stall if the first node can't sync the state from the
#    observer for the shard it doesn't care about
while True:
    assert time.time() - started < TIMEOUT
    status = boot_node.get_status()
    hash_ = status['sync_info']['latest_block_hash']
    new_height = status['sync_info']['latest_block_height']
    seen_boot_heights.add(new_height)
    if new_height > largest_height:
        largest_height = new_height
        print(new_height)
    if new_height >= TWENTY_FIVE:
        break

    if new_height > 1 and not sent_txs:
        ctx.send_moar_txs(hash_, 10, False)
        print("Sending txs at height %s" % new_height)
        sent_txs = True

    time.sleep(0.1)

# 2. Spin up the second node and make sure it gets to 25 as well, and doesn't diverge
node2 = spin_up_node(config, near_root, node_dirs[1], 1, boot_node.node_key.pk, boot_node.addr())

while True:
    assert time.time() - started < TIMEOUT

    status = boot_node.get_status()
    new_height = status['sync_info']['latest_block_height']
    seen_boot_heights.add(new_height)

    if new_height > largest_height:
        largest_height = new_height
        print(new_height)

    status = node2.get_status()
    new_height = status['sync_info']['latest_block_height']
    if new_height > TWENTY_FIVE:
        assert new_height in seen_boot_heights, "%s not in %s" % (new_height, seen_boot_heights)
        break

    time.sleep(0.1)

# 3. During (1) we sent some txs. Make sure the state changed. We can't compare to the
#    expected balances directly, since the tx sent to the shard that node1 is responsible
#    for was never applied, but we can make sure that some change to the state was done,
#    and that the totals match (= the receipts was received)
#    What we are testing here specifically is that the first node received proper incoming
#    receipts during the state sync from the observer.
#    `max_inflation_rate` is set to zero, so the rewards do not mess up with the balances
balances = ctx.get_balances()
print("New balances: %s\nNew total supply: %s" % (balances, sum(balances)))

assert(balances != initial_balances)
assert(sum(balances) == total_supply)

initial_balances = balances

# 4. Stake for the second node to bring it back up as a validator and wait until it actually
#    becomes one

def get_validators():
    return set([x['account_id'] for x in boot_node.get_status()['validators']])

print(get_validators())

tx = sign_staking_tx(node2.signer_key, node2.validator_key, 50000000000000000000000000, 20, base58.b58decode(hash_.encode('utf8')))
boot_node.send_tx(tx)

assert(get_validators() == set(["test0"]))

while True:
    if time.time() - started > TIMEOUT:
        print(get_validators())
        assert False

    if get_validators() == set(["test0", "test1"]):
        break

    time.sleep(1)


ctx.next_nonce = 100
# 5. Record the latest height and bring down the first node, wait for couple epochs to pass
status = node2.get_status()
last_height = status['sync_info']['latest_block_height']

ctx.nodes = [boot_node, node2, observer]
ctx.act_to_val = [1, 1, 1]

boot_node.kill()
seen_boot_heights = set()
sent_txs = False

while True:
    assert time.time() - started < TIMEOUT
    status = node2.get_status()
    hash_ = status['sync_info']['latest_block_hash']
    new_height = status['sync_info']['latest_block_height']
    seen_boot_heights.add(new_height)
    if new_height > largest_height:
        largest_height = new_height
        print(new_height)
    if new_height >= last_height + TWENTY_FIVE:
        break

    if new_height > last_height + 1 and not sent_txs:
        ctx.send_moar_txs(hash_, 10, False)
        print("Sending txs at height %s" % new_height)
        sent_txs = True

    time.sleep(0.1)

balances = ctx.get_balances()
print("New balances: %s\nNew total supply: %s" % (balances, sum(balances)))

ctx.nodes = [observer, node2]
print("Observer sees: %s" % ctx.get_balances())

assert(balances != initial_balances)
assert(sum(balances) == total_supply)

