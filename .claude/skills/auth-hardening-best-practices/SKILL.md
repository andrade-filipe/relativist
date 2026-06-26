---
name: auth-hardening-best-practices
description: Configure rate limiting, manage auth secrets, set up CSRF protection, secure sessions, and implement audit logging for Better Auth deployments. Use when hardening authentication setup or preventing brute force attacks.
license: MIT
---

# Auth Hardening Best Practices (Better Auth)

Comprehensive guide for securing Better Auth deployments, including secret management, rate limiting, and session security.

## Secret Management

### Configuring the Secret

```ts
import { betterAuth } from "better-auth";

export const auth = betterAuth({
  secret: process.env.BETTER_AUTH_SECRET, // or via `BETTER_AUTH_SECRET` env
});
```

Better Auth looks for secrets in this order:
1. `options.secret` in your config
2. `BETTER_AUTH_SECRET` environment variable
3. `AUTH_SECRET` environment variable

### Secret Requirements

- Rejects default/placeholder secrets in production.
- Warns if shorter than 32 characters or entropy below 120 bits.
- Generate: `openssl rand -base64 32`.
- **Never** commit secrets to version control.

## Rate Limiting

Enabled in production by default. Applies to all endpoints.

### Default Configuration

```ts
import { betterAuth } from "better-auth";

export const auth = betterAuth({
  rateLimit: {
    enabled: true, // Default: true in production
    window: 10, // Time window in seconds (default: 10)
    max: 100, // Max requests per window (default: 100)
  },
});
```

### Per-Endpoint Rules

Sensitive endpoints default to 3 requests per 10 seconds. Override for specific paths:

```ts
rateLimit: {
  customRules: {
    "/api/auth/sign-in/email": {
      window: 60, // 1 minute window
      max: 5, // 5 attempts
    },
  },
}
```

## CSRF Protection

Multi-layer protection: origin header validation, Fetch Metadata checks, and first-login protection.

```ts
import { betterAuth } from "better-auth";

export const auth = betterAuth({
  advanced: {
    disableCSRFCheck: false, // Default: false (keep enabled)
  },
});
```

## Trusted Origins

Configure `baseURL` and `trustedOrigins` to prevent unauthorized redirects and origin-based attacks.

```ts
import { betterAuth } from "better-auth";

export const auth = betterAuth({
  baseURL: "https://api.example.com",
  trustedOrigins: [
    "https://app.example.com",
    "https://admin.example.com",
  ],
});
```

## Session and Cookie Security

- **Session Expiration:** Configure `expiresIn` and `updateAge`.
- **Cookie Attributes:** Defaults to `secure: true` (in production), `httpOnly: true`, and `sameSite: "lax"`.
- **Cookie Cache:** Enable `cookieCache` with `strategy: "jwe"` for encrypted session data in cookies.

## Security Auditing

Use `databaseHooks` to implement audit logging for sensitive operations like session creation, email changes, or account linking.

```ts
databaseHooks: {
  user: {
    update: {
      after: async ({ data, oldData }) => {
        if (oldData?.email !== data.email) {
          await auditLog("user.email_changed", { userId: data.id });
        }
      },
    },
  },
}
```

## Security Checklist

- [ ] **Secret:** 32+ characters, high entropy.
- [ ] **HTTPS:** Ensure `baseURL` uses HTTPS.
- [ ] **Rate Limiting:** Keep enabled with appropriate limits.
- [ ] **CSRF:** Keep enabled.
- [ ] **Audit Logging:** Implement for critical events.

---

> Provenance + framework classification: see `composition.yaml` (sidecar).
> Compliance badges: see `badges-draft.yaml` (architect sign-off pending).
