import { AssetKind } from "../../../lib_sdk/asset";
import { createActors } from "../../../lib_sdk/create_actors";

setTimeout(function() {
    describe("Alice can refund before Bob", function() {
        this.timeout(60000);
        it("bitcoin ether", async function() {
            const { alice, bob } = await createActors(
                "e2e-rfc003-btc-eth-inverted-refund.log"
            );

            await alice.sendRequest(AssetKind.Bitcoin, AssetKind.Ether);
            await bob.accept();

            await alice.fund();
            await alice.assertAlphaFunded();
            await bob.assertAlphaFunded();

            await bob.fund();
            await alice.assertBetaFunded();
            await bob.assertBetaFunded();

            await alice.refund();
            await bob.refund();

            await alice.assertBetaRefunded();
            await alice.assertAlphaRefunded();
            await bob.assertBetaRefunded();
            await bob.assertAlphaRefunded();

            await bob.assertRefunded();
            await alice.assertRefunded();
        });
    });
    run();
}, 0);
