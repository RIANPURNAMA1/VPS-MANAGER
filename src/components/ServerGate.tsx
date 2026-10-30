import type { ReactNode } from "react";
import { PlugZap, ServerOff } from "lucide-react";
import { useApp } from "../app/AppContext";
import { Button, Card, EmptyState } from "./ui";

/** Halaman yang butuh server terhubung dibungkus komponen ini. */
export function ServerGate({ children }: { children: (serverId: number) => ReactNode }) {
  const { active, isConnected, connect, go } = useApp();
  if (!active) {
    return (
      <Card><EmptyState icon={<ServerOff className="size-8" />} title="Belum ada server" description="Tambahkan profil VPS terlebih dahulu untuk memakai fitur ini."
        action={<Button variant="primary" onClick={() => go("servers")}>Tambah server</Button>} /></Card>
    );
  }
  if (!isConnected) {
    return (
      <Card><EmptyState icon={<PlugZap className="size-8" />} title={`${active.name} belum terhubung`} description={`Hubungkan ke ${active.username}@${active.host}:${active.port} untuk melanjutkan.`}
        action={<Button variant="primary" onClick={() => void connect(active.id)}>Hubungkan</Button>} /></Card>
    );
  }
  return <>{children(active.id)}</>;
}
