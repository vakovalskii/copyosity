# Cloud Sync API — spec (`services/sync-backend` + Copyosity client)

> Status: **DESIGN / handover**. Implementation not started. This document is the
> contract between the hub (`api.neuraldeep.ru/v1/sync/*`) and the client
> (Copyosity). Review + freeze schemas first, then code on both sides.
> Shape: **separate `services/sync-backend`** (stateful, not a proxy), shares
> hub-postgres + MinIO, auth via resolve-token. MVP: **text + images**.

Handover received 2026-07-10. Client-side checklist lives in §10; do **not**
start implementation until the schemas in §4/§5 are reviewed with the hub team.

## 1. Scope

Cross-device clipboard / snippets sync keyed by one user `sk-...`:

- clipboard entries (text), snippets (text + metadata, folders), images (blobs ≤ ~20 MB).
- Hub stores **ciphertext only** (E2E, §7); `sk-...` = identity/namespace only.
  Encryption uses a **separate passphrase** the hub never sees.

## 2. Infra placement

New `services/sync-backend` (FastAPI), like `ocr-proxy`/`search-proxy` but stateful:
Traefik `PathPrefix(/v1/sync)`; auth via `POST /api/internal/users/resolve-token`;
metadata in hub-postgres (`sync_*` tables); blobs in MinIO bucket `sync`
(content-addressed by SHA-256); quota in Redis DB=4 prefix `sync:` (per-tier, fail-open).

## 3. Auth

Client sends `Authorization: Bearer sk-...` on all `/v1/sync/*`. Backend resolves via
`POST hub.neuraldeep.ru/api/internal/users/resolve-token` (X-Internal-Secret + ipAllowList)
→ `{uid, tier, is_blocked, ...}`. **namespace = `uid`** (extra-keys resolve to owner →
one shared store per account). 60s in-proc cache by salted key hash. `is_blocked` → 403.

## 4. Data model

### 4.1 Server (hub-postgres; all E2E ciphertext except bookkeeping fields)

```sql
CREATE TABLE sync_entry (
  uid          uuid        NOT NULL,
  entry_uuid   uuid        NOT NULL,              -- stable id (client-generated)
  seq          bigserial   NOT NULL,              -- monotonic server cursor (§6)
  kind         text        NOT NULL,              -- 'clip' | 'snippet' | 'folder' | 'image'
  content_hash text        NOT NULL,              -- SHA-256 of plaintext (dedup)
  ciphertext   bytea,                             -- E2E payload (NULL for kind=image)
  blob_ref     text,                              -- SHA-256 of blob in MinIO (kind=image)
  meta_cipher  bytea,                             -- E2E metadata
  deleted      boolean     NOT NULL DEFAULT false,-- tombstone (must replicate!)
  device_id    text,
  client_ts    timestamptz NOT NULL,             -- LWW timestamp
  server_ts    timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (uid, entry_uuid)
);
CREATE INDEX sync_entry_pull ON sync_entry (uid, seq);
CREATE UNIQUE INDEX sync_entry_seq ON sync_entry (uid, seq);

CREATE TABLE sync_blob (
  uid uuid NOT NULL, hash text NOT NULL,          -- SHA-256 of CIPHERTEXT blob
  size_bytes bigint NOT NULL, refcount int NOT NULL DEFAULT 0,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (uid, hash)
);
```

`seq` = per-uid monotonic; MVP uses a global `bigserial`, cursor is still per-uid.

### 4.2 Client (Copyosity local SQLite) — migration adds

`uuid TEXT PK`, `updated_at INTEGER` (unix ms, LWW), `deleted INTEGER`, `device_id TEXT`,
`content_hash TEXT`, `dirty INTEGER` — plus `sync_state(key TEXT PK, value TEXT)`
(`last_pulled_seq`, `passphrase_kdf_salt`, …).

## 5. API (`https://api.neuraldeep.ru/v1/sync`, `Authorization: Bearer sk-...`)

- `POST /push` — upsert delta by `(uid, entry_uuid)`; **LWW by `client_ts`**; losers → `conflicts`;
  `deleted=true` = tombstone; each applied row gets a new `seq`; blob dedup by hash.
  → `{accepted, server_seq, conflicts:[{entry_uuid, winner_seq}]}`.
- `GET /pull?since=<seq>&limit=500` — everything with `seq > since` (incl. tombstones).
  → `{entries:[…{seq}], next_seq, has_more}`. First sync `since=0` (paginate on `has_more`).
- Blobs: `POST /blob/init {hash,size_bytes}` → `{upload_url, exists}` (413 over tier max);
  `GET /blob/<hash>` → `{download_url}`. Client encrypts image E2E **before** hashing/upload;
  `hash` = SHA-256 of ciphertext. `entry(kind=image).blob_ref = hash`. Server refcounts + GC.

## 6. Sync algorithm

1. Push `dirty=1` batched; on 200 clear `dirty`, store `server_seq`; conflicts → re-pull.
2. Pull `since=last_pulled_seq`, apply LWW, store `next_seq`, loop while `has_more`.
3. LWW: larger `client_ts` wins; tie → larger `seq`.
4. Tombstones replicate; client hides/removes locally, keeps tombstone row (~30d GC).
5. Dedup by `content_hash`. 6. Background push/pull on a 15–30s timer + on events.

## 7. E2E encryption (critical — clipboard = passwords/tokens)

- Key derived from a **separate passphrase** (NOT `sk-...`). KDF: `Argon2id(passphrase, salt)`
  (salt stored in `sync_state`, not secret). AEAD: XChaCha20-Poly1305 (or AES-256-GCM),
  random nonce per record; `ciphertext = nonce || AEAD(key, plaintext)`. Same for `meta_cipher`
  and blobs. Hub stores only ciphertext + `content_hash` (SHA-256 of plaintext; use
  `HMAC(key, plaintext)` if the "sameness" leak is unwanted). Passphrase never leaves the client.

## 8. Quotas per-tier (Redis DB=4 `sync:`, fail-open) — DRAFT, confirm with Valera

| tier    | entries | blob storage | max blob |
| ------- | ------- | ------------ | -------- |
| free    | 1 000   | 100 MB       | 5 MB     |
| starter | 20 000  | 2 GB         | 20 MB    |
| pro     | 100 000 | 10 GB        | 20 MB    |
| coder   | 20 000  | 2 GB         | 20 MB    |

Over → `429` (entries/RPS) or `413` (blob) with `{error,limit,tier,upgrade_url}`.

## 9. Errors (client must handle)

`401` invalid key → re-login · `403` blocked → stop · `413` blob too big → "raise tier" +
`upgrade_url` · `429` limit → backoff on `Retry-After` + "raise tier" · `409` version →
re-pull/remerge · `5xx` → exp backoff, never block local work. 429/413 map to the same
"raise your tariff" UX already used for the hub 429 in the client.

## 10. Copyosity client checklist

- [ ] SQLite migration: `uuid/updated_at/deleted/device_id/content_hash/dirty` + `sync_state`.
- [ ] `device_id` generated once at install.
- [ ] E2E: passphrase setup screen (separate from `sk-`), KDF+salt, AEAD encrypt/decrypt.
- [ ] `sync.rs` engine: background push (dirty→server) + pull (since=last_pulled_seq), LWW, dedup, tombstones.
- [ ] Blobs: `blob/init` → PUT ciphertext (if `exists=false`) → `blob_ref`; on pull `GET /blob/<hash>` → decrypt.
- [ ] Settings UI: Sync toggle + passphrase field, reuse existing hub URL + `sk-`.
- [ ] Errors: 429/413 → "raise tier" + `upgrade_url`; 401/403 → re-login/stop.
- [ ] Offline: never block local work; accumulate `dirty`, sync later.

## 11–13

Hub side (`services/sync-backend`): FastAPI skeleton + resolve-token auth, `traefik/dynamic/sync.yml`,
`sync_entry`/`sync_blob` migrations, push/pull with server-seq + LWW + tombstone + dedup,
`blob/init`+`blob/<hash>` signed MinIO URLs + refcount GC, `quota.py`, `SYNC_SERVICE_TOKEN`.
MVP-1: text + images, E2E, server-seq pull, LWW, tombstone, per-tier quota. Then snippets/folders
(`kind=snippet|folder`), then conflict tuning + blob GC, later passphrase rotation + sharing.

Open questions: tier limits (confirm), `content_hash` SHA-256 vs `HMAC(key, …)`, seq global
vs per-uid (MVP=bigserial), blob GC interval/grace.
