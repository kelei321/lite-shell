export interface MonitorSnapshot {
  hostname: string;
  os: string;
  kernel: string;
  uptime: string;
  cpuUsage: number;
  memory: MemorySnapshot;
  swap: MemorySnapshot;
  disks: DiskSnapshot[];
  networks: NetworkSnapshot[];
  collectedAt: number;
}

export interface MemorySnapshot {
  totalMb: number;
  usedMb: number;
  usagePercent: number;
}

export interface DiskSnapshot {
  mount: string;
  filesystem: string;
  total: string;
  used: string;
  available: string;
  usagePercent: number;
}

export interface NetworkSnapshot {
  name: string;
  rxBytes: number;
  txBytes: number;
}
