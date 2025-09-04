import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

export interface DeviceStartPayload { sourceId: string; clientId: string; clientSecret: string }
export interface DeviceStartResponse { user_code: string; verification_url: string; expires_in: number; interval: number }
export type DeviceStatus = "pending" | "authorized" | "error" | "expired";
export interface DeviceStatusResponse { status: DeviceStatus; user_code?: string; verification_url?: string; error?: string }
export interface Album { id: string; title: string; thumbUrl?: string }
export interface AlbumsResponse { albums: Album[]; next_page_token?: string }

const jsonFetch = async <T>(url: string, init?: RequestInit): Promise<T> => {
  const res = await fetch(url, init);
  if (!res.ok) throw new Error(`${init?.method || "GET"} ${url} failed (${res.status})`);
  return res.json() as Promise<T>;
};

export function useDeviceStart(apiBase: string) {
  return useMutation<DeviceStartResponse, Error, DeviceStartPayload>({
    mutationFn: async (p) => jsonFetch<DeviceStartResponse>(`${apiBase}/oauth/google_photos/device/start`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ source_id: p.sourceId, client_id: p.clientId, client_secret: p.clientSecret })
    })
  });
}

export function useDeviceStatus(apiBase: string, sourceId: string, enabled: boolean, refetchMs = 4000) {
  return useQuery<DeviceStatusResponse>({
    queryKey: ['gphotos-device-status', apiBase, sourceId],
    enabled,
    refetchInterval: (query) => {
      if (!enabled) return false;
      const data = query.state.data as DeviceStatusResponse | undefined;
      if (!data) return refetchMs;
      return data.status === 'pending' ? refetchMs : false;
    },
    queryFn: async () => jsonFetch<DeviceStatusResponse>(`${apiBase}/oauth/google_photos/device/status?source_id=${encodeURIComponent(sourceId)}`)
  });
}

export function useAlbums(apiBase: string, sourceId: string, enabled: boolean) {
  return useQuery<Album[]>({
    queryKey: ['gphotos-albums2', apiBase, sourceId],
    enabled,
    queryFn: async () => jsonFetch<Album[]>(`${apiBase}/oauth/google_photos/albums?source_id=${encodeURIComponent(sourceId)}`)
  });
}

export function useAlbumsPaged(apiBase: string, sourceId: string, enabled: boolean, pageToken: string | null, pageSize = 50) {
  return useQuery<AlbumsResponse>({
    queryKey: ['gphotos-albums2', apiBase, sourceId, pageToken, pageSize],
    enabled,
    queryFn: async () => jsonFetch<AlbumsResponse>(`${apiBase}/oauth/google_photos/albums?source_id=${encodeURIComponent(sourceId)}${pageToken ? `&page_token=${encodeURIComponent(pageToken)}`:''}&page_size=${pageSize}`)
  });
}

export function useSetAlbum(apiBase: string, sourceId: string) {
  const qc = useQueryClient();
  return useMutation<void, Error, { albumId: string }>({
    mutationFn: async ({ albumId }) => {
      const res = await fetch(`${apiBase}/sources/${sourceId}/google_photos/album`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ album_id: albumId })
      });
      if (!res.ok) throw new Error('set album failed');
    },
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['gphotos-albums2', apiBase, sourceId] }); }
  });
}

export function useRefreshSource(apiBase: string, sourceId: string) {
  return useMutation<void, Error, void>({
    mutationFn: async () => {
      const res = await fetch(`${apiBase}/sources/${sourceId}/refresh`, { method: 'POST' });
      if (!res.ok) throw new Error('refresh failed');
    }
  });
}
