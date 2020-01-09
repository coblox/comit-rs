import getPort from "get-port";
import * as bitcoin from "./bitcoin";
import { BitcoinNodeConfig } from "./bitcoin";
import { BitcoindInstance } from "./bitcoind_instance";
import { EthereumNodeConfig } from "./ethereum";
import { ParityInstance } from "./parity_instance";

export interface LedgerConfig {
    bitcoin?: BitcoinNodeConfig;
    ethereum?: EthereumNodeConfig;
}

export interface LedgerInstance {
    start(): Promise<LedgerInstance>;
    stop(): void;
}

export class LedgerRunner {
    public readonly runningLedgers: { [key: string]: LedgerInstance };
    private readonly blockTimers: { [key: string]: NodeJS.Timeout };

    constructor(
        private readonly projectRoot: string,
        private readonly logDir: string
    ) {
        this.runningLedgers = {};
        this.blockTimers = {};
    }

    public async ensureLedgersRunning(ledgers: string[]) {
        const toBeStarted = ledgers.filter(name => !this.runningLedgers[name]);

        const promises = toBeStarted.map(async ledger => {
            console.log(`Starting ledger ${ledger}`);

            switch (ledger) {
                case "bitcoin": {
                    const instance = new BitcoindInstance(
                        this.projectRoot,
                        this.logDir,
                        await getPort({ port: 18444 }),
                        await getPort({ port: 18443 })
                    );
                    return {
                        ledger,
                        instance: await instance.start(),
                    };
                }
                case "ethereum": {
                    const instance = new ParityInstance(
                        this.projectRoot,
                        this.logDir,
                        await getPort({ port: 8545 })
                    );
                    return {
                        ledger,
                        instance: await instance.start(),
                    };
                }
                default: {
                    throw new Error(`Ledgerrunner does not support ${ledger}`);
                }
            }
        });

        const startedContainers = await Promise.all(promises);

        for (const { ledger, instance } of startedContainers) {
            this.runningLedgers[ledger] = instance;

            if (ledger === "bitcoin") {
                bitcoin.init(await this.getBitcoinClientConfig());
                this.blockTimers.bitcoin = global.setInterval(async () => {
                    await bitcoin.generate();
                }, 1000);
            }
        }
    }

    public async stopLedgers() {
        const ledgers = Object.entries(this.runningLedgers);

        const promises = ledgers.map(async ([ledger, container]) => {
            console.log(`Stopping ledger ${ledger}`);

            clearInterval(this.blockTimers[ledger]);
            await container.stop();
            delete this.runningLedgers[ledger];
        });

        await Promise.all(promises);
    }

    public async getLedgerConfig(): Promise<LedgerConfig> {
        return {
            bitcoin: await this.getBitcoinClientConfig().catch(() => undefined),
            ethereum: await this.getEthereumNodeConfig().catch(() => undefined),
        };
    }

    private async getBitcoinClientConfig(): Promise<BitcoinNodeConfig> {
        const instance = this.runningLedgers.bitcoin as BitcoindInstance;

        if (instance) {
            const { username, password } = instance.getUsernamePassword();

            return {
                network: "regtest",
                host: "localhost",
                rpcPort: instance.rpcPort,
                p2pPort: instance.p2pPort,
                username,
                password,
            };
        } else {
            return Promise.reject("bitcoin not yet started");
        }
    }

    private async getEthereumNodeConfig(): Promise<EthereumNodeConfig> {
        const instance = this.runningLedgers.ethereum as ParityInstance;

        if (instance) {
            const host = "localhost";
            const port = instance.rpcPort;

            return {
                rpc_url: `http://${host}:${port}`,
            };
        } else {
            return Promise.reject("ethereum not yet started");
        }
    }
}
