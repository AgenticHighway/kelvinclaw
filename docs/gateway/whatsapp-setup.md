# WhatsApp Channel Setup

KelvinClaw supports WhatsApp as an ingress channel via the Meta WhatsApp Cloud API.

## Prerequisites

1. **Meta Developer Account** — create one at [developers.facebook.com](https://developers.facebook.com)
2. **WhatsApp Business App** — create a new app of type "Business" in the Meta Developer dashboard
3. **WhatsApp Cloud API access** — add the WhatsApp product to your app and complete the initial setup

From the Meta Developer dashboard, collect:

| Value                      | Where to find it                                                     |
| -------------------------- | -------------------------------------------------------------------- |
| **Phone Number ID**        | WhatsApp > API Setup > Phone number ID                               |
| **Permanent Access Token** | WhatsApp > API Setup > Generate token (or use a System User token)   |
| **App Secret**             | App Dashboard > Settings > Basic > App Secret                        |
| **Verify Token**           | You choose this — any random string used during webhook registration |

## Environment Variables

```bash
# Required: enable the channel
KELVIN_WHATSAPP_ENABLED=true

# Required: Meta-provided access token for sending outbound messages
KELVIN_WHATSAPP_BOT_TOKEN=<your-access-token>

# Required: your WhatsApp Business phone number ID (for outbound delivery)
KELVIN_WHATSAPP_PHONE_NUMBER_ID=<your-phone-number-id>

# Required for webhook: your Meta App Secret (used for HMAC-SHA256 signature verification)
KELVIN_WHATSAPP_APP_SECRET=<your-app-secret>

# Required for webhook: a token you choose for the webhook verification handshake
KELVIN_WHATSAPP_WEBHOOK_VERIFY_TOKEN=<your-chosen-verify-token>
```

## Webhook Registration

1. Ensure your gateway's HTTP ingress is running and publicly reachable (e.g., behind an ngrok tunnel or a reverse proxy):

```bash
KELVIN_GATEWAY_INGRESS_BIND=0.0.0.0:8080
```

2. In the Meta Developer dashboard, go to **WhatsApp > Configuration > Webhook**:
    - **Callback URL**: `https://your-domain.com/ingress/whatsapp`
    - **Verify Token**: the same value you set in `KELVIN_WHATSAPP_WEBHOOK_VERIFY_TOKEN`
    - Subscribe to `messages` under the Webhook fields

3. Meta will send a GET request with `hub.mode=subscribe`, `hub.verify_token`, and `hub.challenge`. KelvinClaw will verify the token and echo back the challenge, completing the handshake.

## How It Works

1. **Inbound**: When someone sends a WhatsApp message to your business number, Meta posts a webhook event to `/ingress/whatsapp`. KelvinClaw verifies the `X-Hub-Signature-256` HMAC-SHA256 header using your App Secret, extracts the text message, and routes it through the channel engine.

2. **Processing**: The message flows through the standard channel pipeline — deduplication, rate limiting, policy evaluation, routing rules, and WASM policy plugins (if configured).

3. **Outbound**: KelvinClaw sends the response back via `POST https://graph.facebook.com/v21.0/{phone_number_id}/messages` using the Bearer access token.

## Security

- All webhook payloads are verified via HMAC-SHA256 (`X-Hub-Signature-256` header)
- The API base URL is locked to `graph.facebook.com` unless `KELVIN_WHATSAPP_ALLOW_CUSTOM_BASE_URL=true`
- The verify token is only used during the initial webhook registration handshake
- Standard channel policy controls apply: allowlists, trust tiers, rate limits, dedupe

## Common Policy Controls

All standard channel policy env vars work with the `KELVIN_WHATSAPP_` prefix:

```bash
KELVIN_WHATSAPP_ALLOW_ACCOUNT_IDS=+15551234567,+15559876543
KELVIN_WHATSAPP_TRUSTED_SENDER_IDS=+15551234567
KELVIN_WHATSAPP_MAX_MESSAGES_PER_MINUTE=20
KELVIN_WHATSAPP_MAX_TEXT_BYTES=4096
KELVIN_WHATSAPP_INGRESS_TOKEN=<optional-api-ingress-token>
```

## Gateway Methods

| Method                    | Description                                  |
| ------------------------- | -------------------------------------------- |
| `channel.whatsapp.ingest` | Programmatic message injection via WebSocket |
| `channel.whatsapp.status` | Channel health, queue depth, and metrics     |

## Troubleshooting

- **Webhook verification fails**: Check that `KELVIN_WHATSAPP_WEBHOOK_VERIFY_TOKEN` matches what you entered in Meta's dashboard
- **Messages not arriving**: Verify you subscribed to `messages` in the webhook fields and your App Secret is correct
- **Signature mismatch**: Ensure `KELVIN_WHATSAPP_APP_SECRET` is the App Secret (not the access token)
- **Outbound fails**: Check `KELVIN_WHATSAPP_BOT_TOKEN` and `KELVIN_WHATSAPP_PHONE_NUMBER_ID` are set correctly
- Run `kelvin medkit` to diagnose configuration issues
