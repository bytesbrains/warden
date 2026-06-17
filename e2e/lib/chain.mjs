// On-chain Maktub Beat lifecycle for the Veil e2e harness (ethers v6, Base Sepolia).
// Minimal inline ABIs — only what the harness drives. Read-only where it can be.
import { ethers } from "ethers";

const CORE_ABI = [
  "function createHeartbeat(address[] recipients, bytes payload, uint256 interval) payable returns (uint256 id)",
  "function getHeartbeat(uint256 id) view returns (address owner, address[] recipients, bytes payload, uint256 interval, uint256 lastCheckIn, uint256 createdAt, uint256 checkInCount, bool executed, bool deactivated)",
  "function heartbeatCount() view returns (uint256)",
  "function executeHeartbeat(uint256 id)",
  "function deactivate(uint256 id)",
  "function creationFeeFor(uint256 recipientCount) view returns (uint256)",
  "function isExpiredAndActive(uint256 id) view returns (bool)",
  "function MIN_INTERVAL() view returns (uint256)",
];
const REWARDS_ABI = ["function isActiveExecutor(address account) view returns (bool)"];
const REGISTRY_ABI = [
  "function isRegistered(address account) view returns (bool)",
  "function register(bytes encryptionPublicKey)",
];

export function connect(rpc, pk) {
  const provider = new ethers.JsonRpcProvider(rpc);
  const wallet = new ethers.Wallet(pk, provider);
  return { provider, wallet };
}

export function contracts(wallet, { core, rewards, registry }) {
  return {
    core: new ethers.Contract(core, CORE_ABI, wallet),
    rewards: new ethers.Contract(rewards, REWARDS_ABI, wallet),
    registry: new ethers.Contract(registry, REGISTRY_ABI, wallet),
  };
}

// The next Beat id (== current count, pre-increment) so the condition can be built before create.
export async function nextBeatId(core) {
  return (await core.heartbeatCount()).toString();
}

export async function requireExecutor(rewards, address) {
  if (!(await rewards.isActiveExecutor(address))) {
    throw new Error(
      `${address} is not an active executor — stake MKTB first (see scripts/test-heartbeat.js) ` +
        `then re-run. executeHeartbeat is executor-only.`
    );
  }
}

export async function ensureRegisteredRecipient(registry, address) {
  if (await registry.isRegistered(address)) return false;
  // Beat recipients must be registered; the on-chain key is unrelated to the Warden ECIES key.
  const tx = await registry.register(ethers.hexlify(ethers.randomBytes(64)));
  await tx.wait(1);
  return true;
}

export async function createBeat(core, recipients, cid, interval) {
  const payload = ethers.toUtf8Bytes(cid); // store the Warden CID as the Beat payload
  const fee = await core.creationFeeFor(recipients.length);
  const tx = await core.createHeartbeat(recipients, payload, interval, { value: fee });
  await tx.wait(1);
}

export async function status(core, id) {
  const hb = await core.getHeartbeat(id);
  return { executed: hb[7], deactivated: hb[8], interval: hb[3], lastCheckIn: hb[4] };
}

export async function executeBeat(core, id) {
  if (!(await core.isExpiredAndActive(id))) {
    throw new Error(`beat ${id} is not yet expired-and-active; executeHeartbeat would revert`);
  }
  const tx = await core.executeHeartbeat(id);
  await tx.wait(1);
}

export async function deactivateBeat(core, id) {
  const tx = await core.deactivate(id);
  await tx.wait(1);
}
