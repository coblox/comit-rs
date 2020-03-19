import { ChildProcess, spawn } from "child_process";
import * as fs from "fs";
import tmp from "tmp";
import { LogReader } from "./log_reader";
import { promisify } from "util";
import { sleep } from "../utils";
import getPort from "get-port";
import { EthereumInstance } from "./ethereum";
import { Logger } from "log4js";

const openAsync = promisify(fs.open);

export class ParityInstance implements EthereumInstance {
    private process: ChildProcess;
    private dbDir: any;

    public static async new(
        projectRoot: string,
        logFile: string,
        logger: Logger
    ) {
        return new ParityInstance(
            projectRoot,
            logFile,
            logger,
            await getPort({ port: 8545 }),
            await getPort()
        );
    }

    constructor(
        private readonly projectRoot: string,
        private readonly logFile: string,
        private readonly logger: Logger,
        public readonly rpcPort: number,
        public readonly p2pPort: number
    ) {}

    public async start() {
        const bin = process.env.PARITY_BIN
            ? process.env.PARITY_BIN
            : this.projectRoot + "/blockchain_nodes/parity/parity";

        this.logger.info("Using binary", bin);

        this.dbDir = tmp.dirSync();

        this.process = spawn(
            bin,
            [
                `--config=${this.projectRoot}/blockchain_nodes/parity/home/parity/.local/share/io.parity.ethereum/config.toml`,
                `--chain=${this.projectRoot}/blockchain_nodes/parity/home/parity/.local/share/io.parity.ethereum/chain.json`,
                `--base-path=${this.projectRoot}/blockchain_nodes/parity/home/parity/.local/share/io.parity.ethereum`,
                `--db-path=${this.dbDir.name}`,
                `--password=${this.projectRoot}/blockchain_nodes/parity/home/parity/authorities/authority.pwd`,
                `--jsonrpc-port=${this.rpcPort}`,
                `--port=${this.p2pPort}`,
                `--no-ws`,
            ],

            {
                cwd: this.projectRoot,
                stdio: [
                    "ignore", // stdin
                    await openAsync(this.logFile, "w"), // stdout
                    await openAsync(this.logFile, "w"), // stderr
                ],
            }
        );

        this.process.on("exit", (code: number, signal: number) => {
            this.logger.info(
                "parity exited with code",
                code,
                "after signal",
                signal
            );
        });

        const logReader = new LogReader(this.logFile);
        await logReader.waitForLogMessage("Public node URL:");

        this.logger.info("parity started with PID", this.process.pid);
    }

    public get rpcUrl() {
        return `http://localhost:${this.rpcPort}`;
    }

    public async stop() {
        this.logger.info("Stopping parity instance");

        this.process.kill("SIGTERM");
        await sleep(3000);
        this.process.kill("SIGKILL");
    }
}
