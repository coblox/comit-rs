import { Config } from "@jest/types";
import { HarnessGlobal, mkdirAsync } from "./utils";
import NodeEnvironment from "jest-environment-node";
import path from "path";
import { LightningWallet } from "./wallets/lightning";
import { BitcoinWallet } from "./wallets/bitcoin";
import { AssetKind } from "./asset";
import { LedgerKind } from "./ledgers/ledger";
import BitcoinLedger from "./ledgers/bitcoin";
import { BitcoindInstance } from "./ledgers/bitcoind_instance";
import EthereumLedger from "./ledgers/ethereum";
import LightningLedger from "./ledgers/lightning";
import { ParityInstance } from "./ledgers/parity_instance";
import { LndInstance } from "./ledgers/lnd_instance";
import { configure, Logger } from "log4js";

// ************************ //
// Setting global variables //
// ************************ //

export default class E2ETestEnvironment extends NodeEnvironment {
    private docblockPragmas: Record<string, string>;
    private projectRoot: string;
    public global: HarnessGlobal;

    private bitcoinLedger?: BitcoinLedger;
    private ethereumLedger?: EthereumLedger;
    private aliceLightning?: LightningLedger;
    private bobLightning?: LightningLedger;

    private logger: Logger;

    constructor(config: Config.ProjectConfig, context: any) {
        super(config);

        this.docblockPragmas = context.docblockPragmas;
        this.projectRoot = path.resolve(config.rootDir, "..");
    }

    async setup() {
        await super.setup();

        // setup global variables
        this.global.projectRoot = this.projectRoot;
        this.global.ledgerConfigs = {};
        this.global.lndWallets = {};

        const suiteConfig = this.extractDocblockPragmas(this.docblockPragmas);

        const logDir = path.join(
            this.projectRoot,
            "api_tests",
            "log",
            suiteConfig.logDir
        );

        const log4js = configure({
            appenders: {
                multi: {
                    type: "multiFile",
                    base: logDir,
                    property: "categoryName",
                    extension: ".log",
                    layout: {
                        type: "pattern",
                        pattern: "%d %5.10p: %m",
                    },
                },
            },
            categories: {
                default: { appenders: ["multi"], level: "debug" },
            },
        });
        this.global.getLogFile = pathElements =>
            path.join(logDir, ...pathElements);
        this.global.getDataDir = async program => {
            const dir = path.join(logDir, program);
            await mkdirAsync(dir, { recursive: true });

            return dir;
        };
        this.global.getLogger = category => log4js.getLogger(category);
        this.global.parityLockDir = await this.getLockDirectory("parity");

        this.logger = log4js.getLogger("test_environment");
        this.logger.info("Starting up test environment");

        await this.startLedgers(suiteConfig.ledgers);
    }

    /**
     * Initializes all required ledgers with as much parallelism as possible.
     *
     * @param ledgers The list of ledgers to initialize
     */
    private async startLedgers(ledgers: string[]) {
        const startEthereum = ledgers.includes("ethereum");
        const startBitcoin = ledgers.includes("bitcoin");
        const startLightning = ledgers.includes("lightning");

        const tasks = [];

        if (startEthereum) {
            tasks.push(this.startEthereum());
        }

        if (startBitcoin && !startLightning) {
            tasks.push(this.startBitcoin());
        }

        if (startLightning) {
            tasks.push(this.startBitcoinAndLightning());
        }

        await Promise.all(tasks);
    }

    /**
     * Start the Bitcoin Ledger
     *
     * Once this function returns, the necessary configuration values have been set inside the test environment.
     */
    private async startBitcoin() {
        this.bitcoinLedger = await BitcoinLedger.start(
            await BitcoindInstance.new(
                this.projectRoot,
                await this.global.getDataDir("bitcoind"),
                this.logger
            ),
            this.logger,
            await this.getLockDirectory("bitcoind")
        );
        this.global.ledgerConfigs.bitcoin = this.bitcoinLedger.config;
    }
    /**
     * Start the Ethereum Ledger
     *
     * Once this function returns, the necessary configuration values have been set inside the test environment.
     */
    private async startEthereum() {
        this.ethereumLedger = await EthereumLedger.start(
            await ParityInstance.new(
                this.projectRoot,
                this.global.getLogFile(["parity.log"]),
                this.logger
            ),
            this.logger,
            await this.getLockDirectory("parity")
        );
        this.global.ledgerConfigs.ethereum = this.ethereumLedger.config;
        this.global.tokenContract = this.ethereumLedger.config.tokenContract;
    }

    /**
     * First starts the Bitcoin and then the Lightning ledgers.
     *
     * The Lightning ledgers depend on Bitcoin to be up and running.
     */
    private async startBitcoinAndLightning() {
        await this.startBitcoin();

        // Lightning nodes can be started in parallel
        await Promise.all([
            this.startAliceLightning(),
            this.startBobLightning(),
        ]);

        await this.setupLightningChannels();
    }

    private async setupLightningChannels() {
        const { alice, bob } = this.global.lndWallets;

        await alice.connectPeer(bob);

        await alice.mint({
            name: AssetKind.Bitcoin,
            ledger: LedgerKind.Lightning,
            quantity: "15000000",
        });

        await alice.openChannel(bob, 15000000);
    }

    /**
     * Start the Lightning Ledger for Alice
     *
     * This function assumes that the Bitcoin ledger is initialized.
     * Once this function returns, the necessary configuration values have been set inside the test environment.
     */
    private async startAliceLightning() {
        this.aliceLightning = await LightningLedger.start(
            await LndInstance.new(
                await this.global.getDataDir("lnd-alice"),
                "lnd-alice",
                this.logger,
                await this.global.getDataDir("bitcoind")
            ),
            this.logger,
            await this.getLockDirectory("lnd-alice")
        );

        this.global.lndWallets.alice = await LightningWallet.newInstance(
            await BitcoinWallet.newInstance(
                this.bitcoinLedger.config,
                this.logger
            ),
            this.logger,
            this.aliceLightning.config
        );
    }

    /**
     * Start the Lightning Ledger for Bob
     *
     * This function assumes that the Bitcoin ledger is initialized.
     * Once this function returns, the necessary configuration values have been set inside the test environment.
     */
    private async startBobLightning() {
        this.bobLightning = await LightningLedger.start(
            await LndInstance.new(
                await this.global.getDataDir("lnd-bob"),
                "lnd-bob",
                this.logger,
                await this.global.getDataDir("bitcoind")
            ),
            this.logger,
            await this.getLockDirectory("lnd-bob")
        );

        this.global.lndWallets.bob = await LightningWallet.newInstance(
            await BitcoinWallet.newInstance(
                this.bitcoinLedger.config,
                this.logger
            ),
            this.logger,
            this.bobLightning.config
        );
    }

    private async getLockDirectory(process: string): Promise<string> {
        const dir = path.join(this.projectRoot, "api_tests", "locks", process);

        await mkdirAsync(dir, {
            recursive: true,
        });

        return dir;
    }

    async teardown() {
        await super.teardown();
        this.logger.info("Tearing down test environment");

        await this.cleanupAll();
        this.logger.info("Tearing down complete");
    }

    async cleanupAll() {
        const tasks = [];

        if (this.bitcoinLedger) {
            tasks.push(this.bitcoinLedger.stop);
        }

        if (this.ethereumLedger) {
            tasks.push(this.ethereumLedger.stop);
        }

        if (this.aliceLightning) {
            tasks.push(this.aliceLightning.stop);
        }

        if (this.bobLightning) {
            tasks.push(this.bobLightning.stop);
        }

        await Promise.all(tasks);
    }

    private extractDocblockPragmas(
        docblockPragmas: Record<string, string>
    ): { logDir: string; ledgers: string[] } {
        const docblockLedgers = docblockPragmas.ledgers!;
        const ledgers = docblockLedgers ? docblockLedgers.split(",") : [];

        const logDir = this.docblockPragmas.logDir!;
        if (!logDir) {
            throw new Error(
                "Test file did not specify a log directory. Did you miss adding @logDir"
            );
        }

        return { ledgers, logDir };
    }
}
