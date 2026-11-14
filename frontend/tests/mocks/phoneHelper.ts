// Extensions spelled out: `node --test` runs this file directly, without a bundler.
import type { DesktopCommands, DesktopOs, PairedPhone, PhoneHelperStatus } from '../../src/desktop/commands/types.ts';

const PAIR_SECONDS = 120;
const STARTING_MS = 800;
/** After "Pair a phone": a phone starts trying, then pairs. */
const PHONE_TRIES_MS = 5_000;
const PHONE_PAIRS_MS = 2_000;
const CARD_CLOSES_MS = 2_000;
/** After pairing: the phone connects, then starts asking for audio. */
const PHONE_CONNECTS_MS = 1_500;
const PHONE_MAKES_MS = 1_500;

const ADDRESS = '192.168.1.20';
const PHONE_ADDRESS = '192.168.1.34';
const NEW_PHONE: PairedPhone = { peerId: '3fa8c1d29b7e4a60', name: 'Galaxy S10+', state: 'connected', lastSeenAt: null };
const PAIRED_EARLIER: readonly PairedPhone[] = [
  { peerId: '91c0e5f7a2b84d13', name: 'Pixel 8', state: 'offline', lastSeenAt: '2026-11-12T18:40:00.000Z' },
];

export interface MockPhoneHelperOptions {
  os: DesktopOs;
  /** Phones paired before (offline). */
  pairedEarlier?: boolean;
  /** The flows run by themselves: a phone pairs, connects and asks for audio. */
  simulatePhone?: boolean;
  timeScale: number;
}

type PhoneHelperCommands = Pick<
  DesktopCommands,
  | 'phoneHelperStatus' | 'onPhoneHelperStatus' | 'phoneHelperSetEnabled' | 'phoneHelperPairStart' | 'phoneHelperPairCancel' | 'phoneHelperForget'
  | 'phoneHelperRename' | 'phoneHelperSetDownloadVoices' | 'phoneHelperSetPort' | 'phoneHelperDismissModelRequest' | 'phoneHelperDismissVersionMismatch'
  | 'phoneHelperAllowThisNetwork'
>;

/** The phone helper of a shell that exists only in memory. */
export const createMockPhoneHelper = ({ os, pairedEarlier = true, simulatePhone = true, timeScale }: MockPhoneHelperOptions): PhoneHelperCommands => {
  const listeners = new Set<(status: PhoneHelperStatus) => void>();
  let firstListenDone = false;
  let status: PhoneHelperStatus = {
    enabled: false,
    listening: 'off',
    computerName: "Kia's laptop",
    addresses: [],
    notice: os === 'windows' ? { kind: 'windowsFirstListen' } : null,
    network: { kind: 'allowed', name: 'Home Wi-Fi' },
    phones: pairedEarlier ? PAIRED_EARLIER : [],
    pairing: { kind: 'none' },
    modelRequest: null,
    versionMismatch: null,
    downloadVoicesForPhone: false,
    port: 47878,
  };
  const timers = new Set<ReturnType<typeof setTimeout>>();
  const after = (ms: number, step: () => void) => {
    const timer = setTimeout(() => {
      timers.delete(timer);
      step();
    }, ms * timeScale);
    timers.add(timer);
  };
  const stopFlows = () => {
    timers.forEach(clearTimeout);
    timers.clear();
  };
  const update = (patch: Partial<PhoneHelperStatus>) => {
    status = { ...status, ...patch };
    listeners.forEach((listener) => listener(status));
  };
  const setPhone = (peerId: string, patch: Partial<PairedPhone>) => update({ phones: status.phones.map((phone) => (phone.peerId === peerId ? { ...phone, ...patch } : phone)) });
  const listen = () => {
    update({ listening: 'starting', addresses: [] });
    after(STARTING_MS, () => {
      firstListenDone = true;
      update({ listening: 'ready', addresses: [`${ADDRESS}:${status.port}`], notice: null });
    });
  };
  const phoneArrives = () => {
    after(PHONE_TRIES_MS, () => {
      if (status.pairing.kind !== 'showing') return;
      update({ pairing: { ...status.pairing, trying: [PHONE_ADDRESS] } });
      after(PHONE_PAIRS_MS, () => {
        if (status.pairing.kind !== 'showing') return;
        update({ pairing: { kind: 'paired', phoneName: NEW_PHONE.name, peerId: NEW_PHONE.peerId }, phones: [...status.phones.filter((phone) => phone.peerId !== NEW_PHONE.peerId), NEW_PHONE] });
        after(CARD_CLOSES_MS, () => update({ pairing: { kind: 'none' } }));
        after(PHONE_CONNECTS_MS, () => {
          setPhone(NEW_PHONE.peerId, { state: 'connected', lastSeenAt: new Date().toISOString() });
          after(PHONE_MAKES_MS, () => setPhone(NEW_PHONE.peerId, { state: 'making' }));
        });
      });
    });
  };

  return {
    phoneHelperStatus: async () => status,

    onPhoneHelperStatus(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },

    async phoneHelperSetEnabled(enabled) {
      if (!enabled) {
        stopFlows();
        update({
          enabled: false,
          listening: 'off',
          addresses: [],
          pairing: { kind: 'none' },
          notice: os === 'windows' && !firstListenDone ? { kind: 'windowsFirstListen' } : null,
          phones: status.phones.map((phone) => (phone.state === 'offline' ? phone : { ...phone, state: 'offline', lastSeenAt: new Date().toISOString() })),
        });
        return;
      }
      update({ enabled: true });
      listen();
    },

    async phoneHelperPairStart() {
      update({ pairing: { kind: 'showing', code: '482913', expiresAt: Date.now() + PAIR_SECONDS * 1000 * timeScale, totalSeconds: PAIR_SECONDS, wrongCodes: 0, trying: [] } });
      after(PAIR_SECONDS * 1000, () => {
        if (status.pairing.kind === 'showing') update({ pairing: { kind: 'expired' } });
      });
      if (simulatePhone) phoneArrives();
    },

    async phoneHelperPairCancel() {
      stopFlows();
      update({ pairing: { kind: 'none' } });
    },

    async phoneHelperForget(peerId) {
      update({ phones: status.phones.filter((phone) => phone.peerId !== peerId) });
    },

    async phoneHelperRename(name) {
      const trimmed = name.trim();
      if (trimmed.length > 0 && trimmed.length <= 40) update({ computerName: trimmed });
    },

    async phoneHelperSetDownloadVoices(on) {
      update({ downloadVoicesForPhone: on });
    },

    async phoneHelperSetPort(port) {
      update({ port });
      if (status.enabled) listen();
    },

    async phoneHelperDismissModelRequest() {
      update({ modelRequest: null });
    },

    async phoneHelperDismissVersionMismatch() {
      update({ versionMismatch: null });
    },

    async phoneHelperAllowThisNetwork() {
      if (status.network.kind !== 'other') return;
      update({ network: { kind: 'allowed', name: status.network.name } });
      if (status.enabled) listen();
    },
  };
};
