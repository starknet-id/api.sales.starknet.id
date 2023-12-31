import {
  uint256,
  formatUnits,
  Block,
  EventWithTransaction,
  BlockHeader,
} from "./common/deps.ts";
import {
  formatFelt,
  NAMING_CONTRACT,
  SELECTOR_KEYS,
  DECIMALS,
  DB_NAME,
  MONGO_CONNECTION_STRING,
  FINALITY,
} from "./common/constants.ts";
import { decodeDomain } from "./common/starknetid.ts";

const filter = {
  header: { weak: true },
  events: [
    {
      fromAddress: Deno.env.get("NAMING_CONTRACT"),
      keys: [formatFelt(SELECTOR_KEYS.STARK_UPDATE)],
      includeTransaction: true,
      includeReceipt: false,
    },
    {
      fromAddress: Deno.env.get("NAMING_CONTRACT"),
      keys: [formatFelt(SELECTOR_KEYS.SALE_METADATA)],
      includeTransaction: false,
      includeReceipt: false,
    },
    {
      fromAddress: Deno.env.get("ETH_CONTRACT"),
      keys: [formatFelt(SELECTOR_KEYS.TRANSFER)],
      includeTransaction: false,
      includeReceipt: false,
    },
    {
      fromAddress: Deno.env.get("REFERRAL_CONTRACT"),
      keys: [formatFelt(SELECTOR_KEYS.REFERRAL)],
      includeTransaction: false,
      includeReceipt: false,
    },
    {
      fromAddress: Deno.env.get("RENEWAL_CONTRACT"),
      keys: [formatFelt(SELECTOR_KEYS.AUTO_RENEW)],
      includeTransaction: false,
      includeReceipt: false,
    },
  ],
};

export const config = {
  streamUrl: Deno.env.get("STREAM_URL"),
  startingBlock: Number(Deno.env.get("STARTING_BLOCK")),
  network: "starknet",
  filter,
  sinkType: "mongo",
  finality: FINALITY,
  sinkOptions: {
    connectionString: MONGO_CONNECTION_STRING,
    database: DB_NAME,
    collectionName: "sales",
    entityMode: false,
  },
};

type SaleDocument = {
  tx_hash: string;
  meta_hash: string;
  domain: string;
  price: number;
  payer: string;
  timestamp: number;
  expiry: number;
  auto: boolean;
  sponsor?: string;
  sponsor_comm?: number;
};

interface TransferDetails {
  from_address: string;
  amount: string;
}

export default function transform({ header, events }: Block) {
  const { timestamp } = header as BlockHeader;

  let lastTransfer: TransferDetails | null = null;
  let autoRenewed = false;
  let sponsorComm: number | null = null;
  let sponsorAddr: string = "0x0";
  let metadata = "0x0";

  // Mapping and decoding each event in the block
  const decodedEvents = events.map(
    ({ event, transaction }: EventWithTransaction) => {
      const key = BigInt(event.keys[0]);

      switch (key) {
        case SELECTOR_KEYS.TRANSFER: {
          const [fromAddress, toAddress, amountLow, amountHigh] = event.data;
          if (BigInt(toAddress) !== NAMING_CONTRACT) return;

          lastTransfer = {
            from_address: fromAddress,
            amount: formatUnits(
              uint256.uint256ToBN({ low: amountLow, high: amountHigh }),
              DECIMALS
            ),
          };
          break;
        }

        case SELECTOR_KEYS.AUTO_RENEW:
          autoRenewed = true;
          break;

        case SELECTOR_KEYS.REFERRAL:
          sponsorComm = Number(event.data[1]);
          sponsorAddr = event.data[3];
          autoRenewed = true;
          break;

        case SELECTOR_KEYS.SALE_METADATA:
          //domain = Number(event.data[0]);
          metadata = event.data[1];
          break;

        case SELECTOR_KEYS.STARK_UPDATE: {
          if (!lastTransfer) return;

          const arrLen = Number(event.data[0]);
          const expiry = Number(event.data[arrLen + 2]);

          // Basic output object structure
          const output: SaleDocument = {
            tx_hash: transaction.meta.hash,
            meta_hash: metadata.slice(4),
            domain: decodeDomain(event.data.slice(1, 1 + arrLen).map(BigInt)),
            price: +lastTransfer.amount,
            payer: lastTransfer.from_address,
            timestamp: new Date(timestamp).getTime() / 1000,
            expiry,
            auto: autoRenewed,
          };

          // Conditionally add sponsor and sponsor_comm if they are not null
          if (sponsorAddr !== "0x0") {
            output.sponsor = sponsorAddr;
            output.sponsor_comm = +(sponsorComm as number);
          }

          lastTransfer = null;
          autoRenewed = false;
          sponsorComm = null;
          sponsorAddr = "0x0";
          return output;
        }

        default:
          return;
      }
    }
  );

  // Filtering out undefined or null values from the decoded events array
  return decodedEvents.filter(Boolean) as SaleDocument[];
}
