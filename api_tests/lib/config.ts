import tempfile from "tempfile";
import { BitcoinNodeConfig } from "./bitcoin";
import { EthereumNodeConfig } from "./ethereum";
import { LedgerConfig } from "./ledger_runner";

export interface CndConfigFile {
    http_api: HttpApi;
    database?: { sqlite: string };
    network: { listen: string[] };
}

export interface HttpApi {
    socket: { address: string; port: number };
}

export class E2ETestActorConfig {
    public readonly seed: Uint8Array;

    constructor(
        public readonly httpApiPort: number,
        public readonly comitPort: number,
        seed: string,
        public readonly name: string
    ) {
        this.httpApiPort = httpApiPort;
        this.comitPort = comitPort;
        this.seed = new Uint8Array(Buffer.from(seed, "hex"));
    }

    public generateCndConfigFile(ledgerConfig: LedgerConfig): CndConfigFile {
        const dbPath = tempfile(`.${this.name}.sqlite`);
        return {
            http_api: {
                socket: {
                    address: "0.0.0.0",
                    port: this.httpApiPort,
                },
            },
            database: {
                sqlite: dbPath,
            },
            network: {
                listen: [`/ip4/0.0.0.0/tcp/${this.comitPort}`],
            },
            ...createLedgerConnectors(ledgerConfig),
        };
    }
}

interface LedgerConnectors {
    bitcoin?: BitcoinConnector;
    ethereum?: EthereumConnector;
}

interface EthereumConnector {
    node_url: string;
}

interface BitcoinConnector {
    node_url: string;
    network: string;
}

export const ALICE_CONFIG = new E2ETestActorConfig(
    8000,
    9938,
    "f87165e305b0f7c4824d3806434f9d0909610a25641ab8773cf92a48c9d77670",
    "alice"
);
export const BOB_CONFIG = new E2ETestActorConfig(
    8010,
    9939,
    "1a1707bb54e5fb4deddd19f07adcb4f1e022ca7879e3c8348da8d4fa496ae8e2",
    "bob"
);
export const CHARLIE_CONFIG = new E2ETestActorConfig(
    8020,
    8021,
    "6b49ec1df23d124a16d6a12bd34476579e6e80cdcb97a5438cb76ac5c423c937",
    "charlie"
);

function createLedgerConnectors(ledgerConfig: LedgerConfig): LedgerConnectors {
    const config: LedgerConnectors = {};

    if (ledgerConfig.bitcoin) {
        config.bitcoin = bitcoinConnector(ledgerConfig.bitcoin);
    }

    if (ledgerConfig.ethereum) {
        config.ethereum = ethereumConnector(ledgerConfig.ethereum);
    }

    return config;
}

function bitcoinConnector(nodeConfig: BitcoinNodeConfig): BitcoinConnector {
    return {
        node_url: `http://${nodeConfig.host}:${nodeConfig.rpcPort}`,
        network: nodeConfig.network,
    };
}

function ethereumConnector(nodeConfig: EthereumNodeConfig): EthereumConnector {
    return {
        node_url: nodeConfig.rpc_url,
    };
}

export const CND_CONFIGS: {
    [actor: string]: E2ETestActorConfig | undefined;
} = {
    alice: ALICE_CONFIG,
    bob: BOB_CONFIG,
    charlie: CHARLIE_CONFIG,
};
