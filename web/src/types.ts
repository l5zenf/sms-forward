// Type definitions mirroring the axum `/api` JSON contracts. Keeping these
// narrow lets the store fail loudly on shape drift instead of silently
// rendering stale data.

export type SmsStatus =
  | "pending"
  | "sending"
  | "sent"
  | "failed"
  | "decode_failed";

export interface SmsMessage {
  id: number;
  iccid: string | null;
  sender: string | null;
  content: string | null;
  sms_time: string | null;
  received_at: string;
  pdu_raw: string;
  dcs: number | null;
  encoding: string | null;
  concat_ref: string | null;
  concat_total: number | null;
  concat_completed: number;
  modem_mem: string | null;
  modem_index: number | null;
  dedupe_key: string;
  status: string;
  retry_count: number;
  max_retry: number;
  next_retry_at: string | null;
  locked_at: string | null;
  locked_by: string | null;
  forwarded_at: string | null;
  forward_response: string | null;
  last_error: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface MessagePage {
  items: SmsMessage[];
  total: number;
  limit: number;
  offset: number;
}

export interface StatusCounts {
  pending: number;
  sending: number;
  sent: number;
  failed: number;
  decode_failed: number;
  other: number;
  total: number;
}

export interface ModemStatusRecord {
  sim_ready: boolean;
  registered: boolean;
  roaming: boolean;
  csq: number | null;
  rssi_dbm: number | null;
  operator: string | null;
  last_error: string | null;
  updated_at: string | null;
}

export interface ModemEventRecord {
  id: number;
  event_type: string;
  payload: string;
  created_at: string | null;
}

export interface HealthBody {
  status: string;
}
