# API specification

This documents the HTTP contract of `budget-helper-back-r` as actually consumed by
`budget-helper-front` (the Angular app), plus the admin-only `/users` surface that has no
current frontend consumer. It exists so behavior changes are checked against an explicit
contract instead of against "does the Rust code compile" — the bug that prompted this
document (`transaction.account` was `undefined` on the transaction list screen) shipped
because the backend rewrite changed a response shape and nothing asserted what the wire
format was supposed to be.

Each endpoint below is cross-checked against the frontend TypeScript interfaces that consume
it (file paths given). Where no frontend consumer exists, the spec instead reflects the
backend's own internal contract (DB schema + handler behavior), and is marked as such.

Source of truth for frontend types: `budget-helper-front/src/app/**/*.interface.ts`.

## Conventions

- Base path: `/api/v1`.
- Auth: Bearer JWT access token in `Authorization: Bearer <token>`, required on every route
  except `/health` and `/api/v1/auth/*`.
- All request/response bodies are JSON; `Content-Type: application/json` on requests with a
  body.
- Error responses share one envelope, produced by `AppError` (`src/error.rs`), regardless of
  endpoint:

  ```json
  { "message": "string", "status": 404, "timestamp": "2026-08-10T12:00:00+00:00" }
  ```

  | Condition | Status |
  |---|---|
  | Resource not found, or exists but owned by another user | 404 |
  | Missing/invalid/expired/revoked bearer token | 401 |
  | Authenticated but not permitted to act on the resource | 403 |
  | Unique constraint violation (duplicate username/email, MySQL error 1062) | 409 |
  | Deleting an account that still has transactions | 409 |
  | Any other DB or internal error | 500 |

- **Ownership model**: accounts, transactions and budgets are scoped to the authenticated user
  (`user_id` column). Reading/mutating another user's account or transaction by ID returns
  **404**, not 403 — existence isn't leaked. The one exception is creating/updating a
  *transaction* against an `account_id`/`account.id` that belongs to someone else, which
  returns **403** (`src/domain/transaction/handler.rs::resolve_account_for_user`), because at
  that point the account's existence is already implied by the caller having supplied its ID.

---

## Auth — `/api/v1/auth`

No auth required on any route here.

### `POST /api/v1/auth/register`

Frontend consumer: **none currently wired to UI** (`AuthService.register` exists but no
component calls it — see "Known gaps" below for the resulting contract drift).

Request:
```json
{ "username": "string", "role": "ROLE_USER" | "ROLE_ADMIN", "email": "string", "password": "string" }
```
All four fields are required (DB columns are `NOT NULL`); missing any field is a 400/422-level
deserialization failure.

Response `200 OK`:
```json
{ "access": "string", "refresh": "string" }
```
Both are JWTs (HS256, `sub`=username). Registering revokes any previously-issued access tokens
for that user (irrelevant on register since the user is new, but shared code path with
authenticate/refresh — see below).

Errors: `409` if `username` or `email` already exists.

### `POST /api/v1/auth/authenticate`

Frontend consumer: `budget-helper-front/src/app/core/auth/auth.service.ts` (`authenticate`),
via `IAuthenticateRequest`/`IAuthenticateResponse`
(`core/auth/interfaces/authenticate.interface.ts`).

Request:
```json
{ "username": "string", "password": "string" }
```

Response `200 OK`: same shape as register —
```json
{ "access": "string", "refresh": "string" }
```

Side effect: revokes (marks `expired = TRUE, revoked = TRUE`) every previously-issued,
still-live access token for this user, then stores the new one. **Logging in from a second
place invalidates the first's access token** — there is no concept of multiple concurrent
sessions per user.

Errors: `401` if the username doesn't exist or the password doesn't match. (Both cases return
the same generic 401 — the endpoint does not distinguish "unknown user" from "wrong password".)

### `POST /api/v1/auth/refresh`

Frontend consumer: not currently called by the Angular app (no component/interceptor invokes
it yet), but implemented and part of the contract.

Request: no body. The **refresh** token is passed the same way an access token normally would
be — `Authorization: Bearer <refresh_token>`.

Response `200 OK`:
```json
{ "access": "string", "refresh": "string" }
```
`refresh` in the response is an echo of the same refresh token that was sent in (it is not
rotated). Only `access` is newly minted.

Side effect: same revoke-all-then-store-new-access-token behavior as authenticate.

Errors: `401` if the bearer value is missing, not a well-formed/valid JWT, or expired. Note
this endpoint does **not** check the `token` table (`revoked`/`expired` flags) — it only
validates the JWT's signature and its own `exp` claim. A refresh token is never stored in
`token` in the first place, so it can't be revoked server-side (e.g. on logout) even in
principle today.

---

## Account — `/api/v1/account`

All routes require a valid, non-revoked, non-expired access token (`require_auth` middleware),
and every result is scoped to the caller (`user_id`).

Frontend consumer: `budget-helper-front/src/app/modules/account/api/account-api.service.ts`,
types in `modules/account/shared/account.interface.ts`.

### `GET /api/v1/account`

Response `200 OK`:
```json
{
  "accountList": [
    {
      "id": 1,
      "name": "Checking",
      "startingBalance": "500.0000",
      "currentBalance": "1250.0000"
    }
  ]
}
```
Matches `IGetAccountListResponse` **for `id`/`name`**; `startingBalance`/`currentBalance` are
new and not yet declared on `IAccount` — same kind of tracked gap as the transaction list's
`totalElements`/`totalPages` above, until the frontend catches up. `currentBalance` is
**computed on every read**, never stored: `startingBalance` plus every transaction against the
account, where `category: "INCOME"` credits and every other category debits (see
`src/domain/account/repository.rs::SELECT_FIELDS_WITH_BALANCE`). Both are decimal strings at
the same `DECIMAL(19,4)` scale `amount` uses elsewhere. Note: the wire object also carries
`user_id` at the Rust struct level (`AccountDto.user_id`, not `#[serde(skip_serializing)]`) — it
IS present in the JSON even though `IAccount` on the frontend doesn't declare it. Tests should
tolerate its presence (extra field, not `deny_unknown_fields` on this one) rather than assert it
away, since removing it is a separate decision from the scope of this spec.

### `POST /api/v1/account`

Request: `{ "name": "string", "startingBalance": "string" }`. `startingBalance` is optional and
defaults to `"0.0000"` if omitted (`#[serde(default)]`), so existing callers sending only `name`
still work.

Response `200 OK`: `{ "account": { "id": 1, "name": "string", "user_id": 1, "startingBalance": "...", "currentBalance": "..." } }`,
matching `IPostPutAccountResponse` (plus the extra `user_id`/new-fields notes above).

### `GET /api/v1/account/id/{id}`

Response `200 OK`: `{ "account": { "id": ..., "name": ..., "user_id": ..., "startingBalance": ..., "currentBalance": ... } }`
(`IGetAccountResponse`).
Errors: `404` if the account doesn't exist or belongs to another user.

### `PUT /api/v1/account/id/{id}`

Request: `{ "name": "string", "startingBalance": "string" }`. This is a full replace, same as
every other `PUT` in this API — omitting `startingBalance` resets it to `0`, it does not leave
the existing value untouched. A client editing just the name must resend the account's current
`startingBalance` too.
Response `200 OK`: `{ "account": { ... } }` (`IPostPutAccountResponse`).
Errors: `404` if the account doesn't exist or belongs to another user.

### `DELETE /api/v1/account/id/{id}`

Response `200 OK`, empty body.
Errors:
- `404` if the account doesn't exist or belongs to another user.
- `409` if the account still has transactions referencing it (must delete/reassign those
  first).

---

## Budget — `/api/v1/budget`

All routes require auth; results scoped to the caller. A budget is a monthly spending limit for
one category, and is addressed by that category rather than by an ID — there is at most one per
`(user, category)` (`UNIQUE KEY uq_budgets_user_category`), so neither `id` nor `user_id` appears
on the wire. `INCOME` can never carry a budget.

Frontend consumer: `budget-helper-front/src/app/modules/budget/api/budget-api.service.ts`.

The `{category}` path segment is one of `"FOOD"` | `"BILL"` | `"SUBSCRIPTION"` | `"CLOTHING"`
(the same uppercase strings as a transaction's `category`, case-sensitive). `"INCOME"` or any
other value returns `400`.

### `GET /api/v1/budget`

Response `200 OK`, ordered by `category`:
```json
{
  "budgetList": [
    { "category": "FOOD", "monthlyLimit": "300.0000" }
  ]
}
```
`monthlyLimit` is a decimal string at the same `DECIMAL(19,4)` scale as transaction `amount`.
Categories with no budget are simply absent (not returned with a null/zero limit).

### `PUT /api/v1/budget/{category}`

Upsert: creates the caller's budget for `category`, or replaces its limit if one exists.

Request: `{ "monthlyLimit": "300.00" }`.
Response `200 OK`: `{ "budget": { "category": "FOOD", "monthlyLimit": "300.0000" } }`.
Errors: `400` if `category` is unknown or `INCOME`, or `monthlyLimit` is `<= 0`.

### `DELETE /api/v1/budget/{category}`

Response `204 No Content`, empty body.
Errors:
- `400` if `category` is unknown or `INCOME`.
- `404` if the caller has no budget for that category.

---

## Transaction — `/api/v1/transaction`

All routes require auth; results scoped to the caller. This is the domain the reported bug
was in — the response **must** nest a full `account` object, not just `account_id`, or the
Angular transaction list crashes on `element.account.name`.

Frontend consumer: `budget-helper-front/src/app/modules/transaction/api/transaction-api.service.ts`,
types in `modules/transaction/shared/transaction.interface.ts`.

### `GET /api/v1/transaction`

Query params (all optional):

| Param | Format | Default | Notes |
|---|---|---|---|
| `from` | `YYYY-MM-DD` | unbounded | Inclusive lower bound on `date`. |
| `to` | `YYYY-MM-DD` | unbounded | Inclusive upper bound on `date`. `400` if `from` is after `to`. |
| `page` | integer, 0-based | `0` | |
| `size` | integer | `50` | `400` if `0` or `> 200`. |
| `sort` | `"<field>,<direction>"` | `"date,desc"` | `field` is one of `title`\|`account`\|`amount`\|`date`\|`category`; `direction` is `asc`\|`desc`. `400` on an unknown field/direction or a value with no comma. |

Response `200 OK`:
```json
{
  "transactionList": [
    {
      "id": 1,
      "category": "FOOD" | "BILL" | "SUBSCRIPTION" | "CLOTHING" | "INCOME",
      "title": "string",
      "amount": "84.32",
      "date": "2026-08-08",
      "account": { "id": 1, "name": "Checking", "user_id": 1 }
    }
  ],
  "totalElements": 1,
  "totalPages": 1
}
```
Matches `IGetTransactionListResponse` / `ITransaction` **for the `transactionList` array only**
— `totalElements`/`totalPages` are new and the frontend interface doesn't declare them yet, so
this is a known, tracked contract gap until the frontend's pagination work lands (extra JSON
fields are harmless to an untyped consumer, but nothing on the frontend can use these two until
`IGetTransactionListResponse` is updated to match).
`account` **must** be a nested object with `.name` — this is the field the reported crash read
`undefined.name` from.
`amount` is serialized as a decimal string (via `rust_decimal`), not a JSON number, and is
normalized to the `amount DECIMAL(19,4)` column's scale on every read — a transaction created
with `"amount": "42.10"` reads back as `"42.1000"`, regardless of how the client formatted it
going in.
`date` is `YYYY-MM-DD`.

### `POST /api/v1/transaction`

Request:
```json
{
  "category": "FOOD" | "BILL" | "SUBSCRIPTION" | "CLOTHING" | "INCOME",
  "title": "string",
  "amount": "12.34",
  "date": "2026-08-08",
  "account": { "id": 1 }
}
```
Matches what `transaction-card.component.ts` actually sends (`ITransaction`, minus `id`): the
account is sent as a **nested object**, not a bare `account_id`. Only `account.id` is read by
the backend; extra fields (e.g. `account.name`, if the frontend includes it) are ignored.

Response `200 OK`: `{ "transaction": { ...same shape as list item... } }`
(`IPostPutTransactionResponse`).

Errors:
- `403` if `account.id` refers to an account owned by a different user.
- `404` if `account.id` doesn't exist at all (account lookup fails before the ownership check).

### `GET /api/v1/transaction/id/{id}`

Response `200 OK`: `{ "transaction": { ... } }` (`IGetTransactionResponse`).
Errors: `404` if the transaction doesn't exist or belongs to another user.

### `PUT /api/v1/transaction/id/{id}`

Request: same shape as `POST`.
Response `200 OK`: `{ "transaction": { ... } }` (`IPostPutTransactionResponse`).
Errors:
- `403` if the new `account.id` belongs to another user.
- `404` if the transaction itself doesn't exist / isn't owned by the caller, or if `account.id`
  doesn't exist.

### `DELETE /api/v1/transaction/id/{id}`

Response `200 OK`, empty body.
Errors: `404` if the transaction doesn't exist or belongs to another user.

---

## Users (admin) — `/api/v1/users`

**No current frontend consumer** — there is no admin UI module in `budget-helper-front`. This
section documents the backend's actual behavior as a baseline for regression tests, not a
contract validated against a real client.

**Access control**: every route below requires the caller to be authenticated *and* hold
`ROLE_ADMIN`. This is enforced by `require_admin` (`src/middleware/auth.rs`), layered on the
`admin_only` sub-router in `src/app.rs` *inside* `require_auth` — meaning `require_auth` runs
first (so a missing/invalid token still gets `401`, not `403`), and `require_admin` runs second
and rejects any authenticated `ROLE_USER` caller with `403`. This was previously unenforced
(any authenticated user could act on any other user's account); see "Known gaps" below for a
consequence of closing that hole that's still open.

### `GET /api/v1/users`

Response `200 OK`: a bare JSON array (not wrapped in an object, unlike every other list
endpoint in this API):
```json
[ { "id": 1, "username": "string", "role": "ROLE_USER", "email": "string" }, ... ]
```
`password` is present on the `User` struct but `#[serde(skip_serializing)]`, so it never
appears in output.

### `POST /api/v1/users`

Request: `{ "username": "string", "role": "ROLE_USER" | "ROLE_ADMIN", "email": "string", "password": "string" }`.
Response `200 OK`: the created user, same shape as a list item (bare object, no wrapper).
Errors: `409` on duplicate username/email.

### `GET /api/v1/users/{id}`

Response `200 OK`: bare user object. Errors: `404` if no user with that ID exists (this
endpoint is **not** scoped to the caller — any authenticated user can fetch any user by ID).

### `PUT /api/v1/users/{id}`

Request: full user object including `password` — **every update requires resending a
password**, since the backend re-hashes whatever `password` value is in the payload
unconditionally (`user::handler::update`). There's no partial-update path.
Response `200 OK`: bare updated user object.

### `DELETE /api/v1/users/{id}`

Response `200 OK`, empty body. Errors: `404` if no user with that ID exists.

---

## Known gaps (documented, not silently fixed)

1. **`register` request shape mismatch.** Backend requires `{username, role, email, password}`;
   the frontend's `IRegisterRequest` (`core/auth/interfaces/register.interface.ts`) only has
   `{username, password}`. If a signup UI is ever wired to `AuthService.register`, it will
   fail deserialization (missing `role`/`email`) until one side changes. No component calls
   `register()` today, so this hasn't manifested as a user-visible bug yet.
2. **`register` silently ignores whatever `role` it's sent.** `auth::handler::register`
   hardcodes every new account to `Role::RoleUser` regardless of `RegisterRequest.role` — a
   *good* thing security-wise (self-service signup can never mint an admin), but it does mean
   the field is accepted and then thrown away without any error, which is confusing API
   behavior worth documenting explicitly.
3. **`GET /api/v1/transaction`'s new `totalElements`/`totalPages` fields aren't in
   `IGetTransactionListResponse` yet.** The frontend still fetches the whole list unfiltered
   and paginates/sorts client-side (`transaction-list.component.ts`'s `MatTableDataSource`);
   the backend now supports `from`/`to`/`page`/`size`/`sort`, but nothing on the frontend sends
   them or reads the new response fields. Not a wire-format break (unknown JSON fields are
   harmless to an untyped consumer) — just unfinished, tracked here until the frontend half of
   this change lands.
4. **Account `startingBalance`/`currentBalance` aren't in `IAccount` yet, and `"INCOME"` isn't
   in the frontend's `Category` union.** The account create/edit form only sends `{ "name" }`
   today (`startingBalance` defaults to `0`, per the endpoint docs above), the account list
   doesn't display a balance column, and the transaction category dropdown can't produce
   `"INCOME"` — so nothing on the frontend can currently push a balance away from
   `-Σ(expenses)`. Backend-complete, frontend-pending, same pattern as gap 3.

## Bootstrapping the first admin

Since `register` can never produce a `ROLE_ADMIN` account (gap 2 above) and `/api/v1/users` is
itself admin-gated, `bootstrap_admin::run` (`src/bootstrap_admin.rs`) runs once on every
startup, right after migrations and before `dev_seed`, in every profile:

- If any `ROLE_ADMIN` row already exists, it's a no-op. This makes it safe to leave
  `bootstrap_admin_*` set permanently — it only ever does anything on a fresh database's
  first run.
- Otherwise, if `bootstrap_admin_username`, `bootstrap_admin_email`, and
  `bootstrap_admin_password` are all set, it creates that admin.
- If none of the three are set, it logs a warning and continues — the app still starts, but
  `/api/v1/users` is unreachable until an admin exists by some other means.
- If only some of the three are set, or the username/email collides with an existing
  non-admin account, it logs a warning and skips rather than failing startup or silently
  promoting an unrelated account.

There is deliberately no default admin/password shipped in `config/application-dev.yaml` —
bootstrapping is opt-in even within dev. In production, `bootstrap_admin_*` is written into
the gitignored `config/application-prod.yaml` by the deployment process (e.g. pulled from a
secrets manager) rather than committed — see `config/application-prod.yaml.example`.

## What this spec does *not* cover

- `/health` (trivial, plain-text `"ok"`, not part of the versioned API).
- Exact SQL error taxonomy for `AppError::Database` beyond the 1062-duplicate-key special
  case.
