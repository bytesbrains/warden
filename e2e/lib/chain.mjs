// On-chain Maktub Beat lifecycle for the Veil e2e harness (ethers v6, Base Sepolia).
// Minimal inline ABIs — only what the harness drives. Read-only where it can be.
import { ethers } from "ethers";

const CORE_ABI = [
  // D-038: creator-chosen deterministic id = keccak256(abi.encode(sender, salt)).
  "function createHeartbeat(bytes32 salt, address[] recipients, bytes payload, uint256 interval) payable returns (uint256 id)",
  "function getHeartbeat(uint256 id) view returns (address owner, address[] recipients, bytes payload, uint256 interval, uint256 lastCheckIn, uint256 createdAt, uint256 checkInCount, bool executed, bool deactivated)",
  "function heartbeatCount() view returns (uint256)",
  "function execute(uint256 id)",
  "function deactivate(uint256 id)",
  "function creationFeeFor(uint256 recipientCount) view returns (uint256)",
  "function isExpiredAndActive(uint256 id) view returns (bool)",
  "function MIN_INTERVAL() view returns (uint256)",
  "function EXECUTION_GRACE() view returns (uint256)", // D-040: permissionless backstop window
  "function getInboxBeats(address recipient) view returns (uint256[])", // D-038 recipient discovery
  "function getOwnerBeats(address owner) view returns (uint256[])", // D-038 owner discovery
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

// A fresh random 32-byte salt — the per-beat uniquifier for the content-addressed id (D-038).
export function randomSalt() {
  return ethers.hexlify(ethers.randomBytes(32));
}

// The deterministic Beat id the contract derives for (sender, salt): keccak256(abi.encode(...)),
// as a decimal string (the form the Warden condition's `args` carry). Known BEFORE create — so
// the Veil condition can be sealed to this beat's own id with no counter race (D-038).
export function beatId(sender, salt) {
  const h = ethers.keccak256(
    ethers.AbiCoder.defaultAbiCoder().encode(["address", "bytes32"], [sender, salt])
  );
  return BigInt(h).toString();
}

// Recipient discovery (D-038): the beat ids where `recipient` is (or was) a recipient.
export async function inboxBeats(core, recipient) {
  return (await core.getInboxBeats(recipient)).map((x) => x.toString());
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

export async function createBeat(core, salt, recipients, cid, interval) {
  const payload = ethers.toUtf8Bytes(cid); // store the Warden CID as the Beat payload
  const fee = await core.creationFeeFor(recipients.length);
  const tx = await core.createHeartbeat(salt, recipients, payload, interval, { value: fee });
  await tx.wait(1);
}

export async function status(core, id) {
  const hb = await core.getHeartbeat(id);
  return { executed: hb[7], deactivated: hb[8], interval: hb[3], lastCheckIn: hb[4] };
}

export async function executeBeat(core, id) {
  if (!(await core.isExpiredAndActive(id))) {
    throw new Error(`beat ${id} is not yet expired-and-active; execute would revert`);
  }
  const tx = await core.execute(id);
  await tx.wait(1);
}

export async function deactivateBeat(core, id) {
  const tx = await core.deactivate(id);
  await tx.wait(1);
}
