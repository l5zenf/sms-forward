// Thin fetch wrapper around the `/api` endpoints. Each function aborts cleanly
// when the caller passes an AbortSignal so polled requests can be superseded.

import type {
  HealthBody,
  MessagePage,
  ModemEventRecord,
  ModemStatusRecord,
  SmsMessage,
  StatusCounts,
} from "./types";

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
    this.name = "ApiError";
  }
}

async function getJson<T>(path: string, signal?: AbortSignal): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, {
      signal,
      headers: { Accept: "application/json" },
    });
  } catch (e) {
    // Network failure / server down — surface a readable message so the UI can
    // show an offline banner instead of a generic "failed to fetch".
    if ((e as Error).name === "AbortError") throw e;
    throw new ApiError(0, "无法连接到服务器");
  }
  if (!res.ok) {
    let message = `请求失败 (${res.status})`;
    try {
      const body = (await res.json()) as { message?: string };
      if (body?.message) message = body.message;
    } catch {
      /* keep default */
    }
    throw new ApiError(res.status, message);
  }
  return (await res.json()) as T;
}

export const api = {
  health: (signal?: AbortSignal) => getJson<HealthBody>("/api/health", signal),
  stats: (signal?: AbortSignal) => getJson<StatusCounts>("/api/stats", signal),
  modemStatus: (signal?: AbortSignal) =>
    getJson<ModemStatusRecord | null>("/api/modem/status", signal),
  modemEvents: (limit: number, signal?: AbortSignal) =>
    getJson<ModemEventRecord[]>(
      `/api/modem/events?limit=${encodeURIComponent(limit)}`,
      signal,
    ),
  messages: (
    opts: { limit?: number; offset?: number; status?: string; q?: string },
    signal?: AbortSignal,
  ) => {
    const params = new URLSearchParams();
    if (opts.limit != null) params.set("limit", String(opts.limit));
    if (opts.offset != null) params.set("offset", String(opts.offset));
    if (opts.status) params.set("status", opts.status);
    if (opts.q) params.set("q", opts.q);
    const qs = params.toString();
    return getJson<MessagePage>(`/api/messages${qs ? `?${qs}` : ""}`, signal);
  },
  message: (id: number, signal?: AbortSignal) =>
    getJson<SmsMessage | null>(`/api/messages/${id}`, signal),
};
