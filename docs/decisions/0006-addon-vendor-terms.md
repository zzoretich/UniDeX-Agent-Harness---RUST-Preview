# 0006: Add-on vendor terms (Claude Code and Codex over ACP)

## Status

Proposed, awaiting the maintainer's read. The maintainer reads this record before milestone M5 is planned.

## Date

2026-11-16. Every page cited below was fetched and read on this date.

## Context

UniDeX Rust Preview is an MIT desktop agent harness with its own engine, which calls model APIs with the user's own API
keys or local models (spec section 4.1). As optional add-ons it can drive an external agent the user has installed and
signed in to themselves, Claude Code or Codex, by spawning that agent's Agent Client Protocol (ACP) adapter as a child
process on the user's machine (spec section 5.4). UniDeX never bundles or installs an agent or adapter, never offers a
vendor login screen, never handles the vendor login, and never proxies or resells access. Spec section 3 assumes that
frontier-model subscriptions are reachable only through the vendors' own tools and asks for each add-on's terms to be
checked in M0. Spec section 5.4 says an agent whose terms rule it out is left off the list, and the ACP engine ships
regardless.

The question for each agent: what do the vendor's current documentation and terms say about a third-party application
driving the user's own installed, signed-in CLI through an ACP adapter, and which billing pool that use draws on.

Method: only primary sources were used: the vendors' own documentation, terms, policies, help-centre articles and
pricing pages, the adapters' own repositories, and the Agent Client Protocol's own site. Search results and news
articles were used only to find pages; nothing below rests on them. Where the sources read do not answer a point, the
record says "not stated in the sources read", and no permission is inferred from silence. One short direct quotation is
used; everything else is paraphrase.

This is an engineering record of what the cited pages say on the date read; it is not legal advice.

## Sources read

All dates read: 2026-11-16. IDs are used in the rest of this record.

| ID | Source | Address | Date read | What it was used for |
|---|---|---|---|---|
| A1 | Claude Code docs: Legal and compliance | https://code.claude.com/docs/en/legal-and-compliance | 2026-11-16 | Which terms govern Claude Code; authentication and credential rules for third-party developers; conditions for running Claude Code in a product; name and logo rules |
| A2 | Claude Code docs: Agent SDK overview | https://code.claude.com/docs/en/agent-sdk/overview | 2026-11-16 | No claude.ai login or rate limits in third-party products; branding guidelines; Commercial Terms govern the SDK |
| A3 | Claude Code docs: Agent SDK quickstart | https://code.claude.com/docs/en/agent-sdk/quickstart | 2026-11-16 | Supported SDK credentials (API key, cloud providers); repeats the A2 notice |
| A4 | Claude Code docs: Authentication | https://code.claude.com/docs/en/authentication | 2026-11-16 | Credential precedence; environment credentials also apply to the Agent SDK |
| A5 | Claude Code docs index | https://code.claude.com/docs/llms.txt | 2026-11-16 | Searched for ACP, Agent Client Protocol and Zed: no page found |
| A6 | Claude Help Center: Log in to your Claude account (page updated 2026-11-07) | https://support.claude.com/en/articles/13189465-log-in-to-your-claude-account | 2026-11-16 | Subscription use by third-party tools, including open-source projects; usage credits; prohibited tool behaviour |
| A7 | Claude Help Center: Use the Claude Agent SDK with your Claude plan (page updated 2026-10-30) | https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan | 2026-11-16 | Billing pool for Agent SDK, `claude -p` and third-party app use on a plan |
| A8 | Claude Help Center: Monthly API credits for Max, Ultra, and Team plans (page updated 2026-10-30) | https://support.claude.com/en/articles/17154008-monthly-api-credits-for-max-ultra-and-team-plans | 2026-11-16 | Which pool Agent SDK use draws on with a plan sign-in versus an API key; the June credit pool is not available |
| A9 | Claude Help Center: Use Claude Code with your Pro, Max, or Ultra plan (page updated 2026-10-30) | https://support.claude.com/en/articles/11145838-use-claude-code-with-your-pro-max-or-ultra-plan | 2026-11-16 | Plan limits; an API key in the environment bills API rates instead of the plan |
| A10 | Claude Help Center: Buy usage bundles (page updated 2026-11-06) | https://support.claude.com/en/articles/14246112-buy-usage-bundles | 2026-11-16 | Bundle balance also covers third-party products that use a Claude account |
| A11 | Claude Help Center: Manage usage credits for paid Claude plans (page updated 2026-11-10) | https://support.claude.com/en/articles/12429409-manage-extra-usage-for-paid-claude-plans | 2026-11-16 | What usage credits are (pay as you go after plan limits) |
| A12 | Anthropic Consumer Terms of Service (effective 2025-10-08) | https://www.anthropic.com/legal/consumer-terms | 2026-11-16 | Automated access only by API key or explicit permission; no sharing of credentials |
| A13 | Anthropic Commercial Terms of Service (effective 2025-06-17) | https://www.anthropic.com/legal/commercial-terms | 2026-11-16 | Govern API keys and the Agent SDK; resale restriction; nothing specific to Claude Code |
| A14 | Anthropic Usage Policy (effective 2026-11-12) | https://www.anthropic.com/legal/aup | 2026-11-16 | Prohibits services that route requests through consumer subscriptions or misrepresent the client |
| O1 | Codex docs: Authentication (developers.openai.com/codex/auth redirects here) | https://learn.chatgpt.com/docs/auth | 2026-11-16 | ChatGPT sign-in versus API key; API key billing; cached login shared by the CLI and extension |
| O2 | Codex docs: Codex App Server | https://learn.chatgpt.com/docs/app-server | 2026-11-16 | Policy on local, open-source and commercial apps using app-server authentication; client identification |
| O3 | Codex docs: Codex SDK | https://learn.chatgpt.com/docs/codex-sdk | 2026-11-16 | Integrating Codex into your own application; custom clients pointed to the app server |
| O4 | Codex docs: Pricing | https://learn.chatgpt.com/docs/pricing | 2026-11-16 | Plan usage, credits and API-key billing for Codex |
| O5 | Codex docs: Open Source, and the Codex docs index | https://learn.chatgpt.com/docs/open-source and https://developers.openai.com/codex/llms.txt | 2026-11-16 | OpenAI's list of its open-source Codex components; searched for ACP and Zed: none found |
| O6 | Sign in with ChatGPT docs: index and Quickstart | https://developers.openai.com/siwc and https://developers.openai.com/siwc/quickstart | 2026-11-16 | OpenAI's sanctioned route for third-party apps to use a ChatGPT plan; who it is open to |
| O7 | Sign in with ChatGPT: ChatGPT plan usage overview | https://developers.openai.com/siwc/token-sharing-open-source | 2026-11-16 | Plan usage for open-source and locally hosted apps; paid or remotely hosted apps need the interest form |
| O8 | Sign in with ChatGPT: Codex app-server | https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server | 2026-11-16 | How an app drives Codex app-server with its own plan-usage token; client identification |
| O9 | Sign in with ChatGPT: Preview limitations | https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations | 2026-11-16 | Limits of plan usage through Codex app-server |
| O10 | Sign in with ChatGPT: UI/UX guidelines | https://developers.openai.com/siwc/ui-ux-guidelines | 2026-11-16 | How OpenAI wants plan use shown to users |
| O11 | Sign in with ChatGPT: Request a client ID | https://developers.openai.com/siwc/request-client-id | 2026-11-16 | Commercial sign-in is waitlisted |
| O12 | OpenAI Help Center: Using Codex with your ChatGPT plan (page said it was updated 12 days before the read date) | https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan | 2026-11-16 | Plan availability, shared allowance and credit pool, which terms apply |
| O13 | OpenAI Terms of Use (effective 2026-01-01) | https://openai.com/policies/terms-of-use/ | 2026-11-16 | No sharing credentials; no circumventing rate limits; name and logo use |
| O14 | OpenAI Service Terms (updated 2026-11-13) | https://openai.com/policies/service-terms/ | 2026-11-16 | Codex section; Sign in with ChatGPT section |
| O15 | OpenAI Usage Policies (page shows a version effective 2026-11-20) | https://openai.com/policies/usage-policies/ | 2026-11-16 | Checked for anything on third-party clients: nothing found |
| O16 | OpenAI Design (brand) Guidelines | https://openai.com/brand/ | 2026-11-16 | No implied endorsement; no OpenAI logo in own branding |
| O17 | OpenAI Sign in with ChatGPT Terms (effective 2026-11-04) | https://openai.com/policies/sign-in-with-chatgpt-terms/ | 2026-11-16 | Obligations an app takes on if it uses Sign in with ChatGPT plan usage (open point only) |
| R1 | Repository agentclientprotocol/claude-agent-acp: README, LICENSE, package.json, `src/auth-methods.ts`, `src/hide-claude-auth.ts`, `src/v2/subscription-guard.ts`, repository metadata | https://github.com/agentclientprotocol/claude-agent-acp | 2026-11-16 | Claude adapter: maintainer, licence, how it runs Claude Code, authentication, subscription guard flag |
| R2 | Repository agentclientprotocol/codex-acp: README, LICENSE, package.json, `src/CodexAuthMethod.ts`, `src/CodexAcpClient.ts`, repository metadata | https://github.com/agentclientprotocol/codex-acp | 2026-11-16 | Codex adapter: maintainer, licence, how it runs Codex, authentication, client identity |
| R3 | Repository zed-industries/codex-acp (archived): README, LICENSE | https://github.com/zed-industries/codex-acp | 2026-11-16 | Earlier Codex adapter; points to R2 |
| R4 | Agent Client Protocol: Agents | https://agentclientprotocol.com/get-started/agents | 2026-11-16 | How the protocol's site lists the two adapters |
| R5 | Agent Client Protocol: Governance | https://agentclientprotocol.com/community/governance | 2026-11-16 | Who governs ACP |
| R6 | Agent Client Protocol: Authentication (v1) | https://agentclientprotocol.com/protocol/v1/authentication | 2026-11-16 | Agent-handled versus terminal login in ACP |

Pages not reached or not read: the openai.com policy pages (O13 to O17) refused a plain HTTP fetch (status 403) and were
read in a full browser instead, so they were reached. Not read: the version of the OpenAI Usage Policies in force before
2026-11-20 (O15 already shows the next version), and Anthropic's Trademark Guidelines (A1 summarises the relevant rule).

## Findings for Claude Code

### 1. Third-party applications driving the user's own CLI or its SDK

- Governing terms. Claude Code is under the Consumer Terms for Free, Pro, Max and Ultra users and the Commercial Terms
  for Team, Enterprise and API users (A1). The Agent SDK is under the Commercial Terms, including when it powers
  products offered to others (A2).
- Authentication rules (A1). OAuth sign-in is meant only for subscription purchasers and for ordinary use of Claude Code
  and other native Anthropic applications. Developers building products that interact with Claude, explicitly including
  products built on the Agent SDK, should use API keys from Claude Console or a supported cloud provider. Anthropic does
  not permit third-party developers to offer claude.ai login in their applications, or to route requests through Free,
  Pro, Max or Ultra credentials on behalf of their users. Developers may not collect, store or intermediate claude.ai
  credentials or session tokens, and sign-in must complete through Anthropic's own flow. Anthropic says it may enforce
  this without notice.
- Carve-outs (A1). The rules do not restrict customers provisioning their own API keys, provided usage is billed to the
  key owner and not resold. They also do not stop an end user signing in to the unmodified Claude Code binary with their
  own subscription, including where a platform hosts Claude Code under the conditions in the next point.
- Running Claude Code inside a product (A1). This requires agreeing to the Commercial Terms; the binary must be
  installed and run as published; no built-in authentication method may be removed, disabled or restricted (claude.ai
  sign-in included); the product may not pay for, resell or intermediate usage; each end user authenticates with their
  own credentials and is billed directly.
- Agent SDK pages (A2, A3). Unless Anthropic has approved it beforehand, third-party developers may not offer claude.ai
  login or rate limits in their products, agents built on the Agent SDK included; they should use the API-key methods in
  the quickstart (Anthropic API key, or Amazon Bedrock, Claude Platform on AWS, Google Cloud's Agent Platform, Microsoft
  Foundry).
- Help Center (A6, updated 2026-11-07). Plan usage is designed for ordinary use of Anthropic's own applications. The
  preferred route for third-party tools, open-source projects included, is API-key authentication. Anthropic may, at its
  discretion, let paid subscribers who have turned on usage credits use certain third-party tools, and may bill that use
  to usage credits instead of plan limits. Tools that misrepresent their identity to Anthropic's servers or try to route
  third-party traffic against subscription limits are prohibited. Anyone building a tool for others is told to use API
  keys.
- Usage Policy (A14, effective 2026-11-12). Reselling, proxying or otherwise providing access to Claude through
  unauthorized means is prohibited, naming services that route requests through consumer subscriptions or misrepresent
  the product or client in use.
- Consumer Terms (A12). Automated or non-human access is not allowed except by an Anthropic API key or where Anthropic
  explicitly permits it; account credentials may not be shared.
- Statements pointing the other way. A7 (its June 2026 update, still on the page) says Agent SDK, `claude -p` and
  third-party app usage still draw from subscription limits. A10 says the usage-bundle balance also covers third-party
  products that use a Claude account. A1 says advertised plan limits assume ordinary, individual use of Claude Code and
  the Agent SDK.
- ACP and the adapter: not stated in the sources read. No Anthropic page read mentions ACP, Zed or the Claude ACP
  adapter (A5). Which third-party tools Anthropic allows on a plan under A6: not stated in the sources read.

Reading. API-key, Console and cloud-provider credentials through a third-party tool: permitted by every page read.
claude.ai subscription credentials through a third-party tool built on the Agent SDK: not clearly permitted. The most
specific pages (A1, A2, A6) direct such tools to API keys and say a plan's limits may not be offered without approval;
A14 adds a prohibition on routing through consumer subscriptions. The only route that might cover it is A1's carve-out
for an end user signing in to the unmodified binary on a platform that runs Claude Code, and whether a desktop app plus
an Agent-SDK-based adapter is such a platform is not settled by the sources.

### 2. Billing pool

- Subscription user signed in with a plan, no API key present: Agent SDK and `claude -p` use draws from the plan's usage
  limits (A7, A8). A6 reserves Anthropic's right to bill third-party tool use to usage credits instead (pay as you go at
  API rates, A11); which tools that applies to is not stated in the sources read. The Max, Ultra and Team monthly API
  credits do not cover plan-signed-in use (A8). The separate Agent SDK credit announced in June is not available (A8).
- API-key user: billed to the key owner at standard API rates (A1, A9). The Max, Ultra and Team monthly API credits do
  cover the Agent SDK and `claude -p` when run with an API key from the Console organization linked to the plan (A8). An
  `ANTHROPIC_API_KEY` in the environment outranks a subscription login, and in non-interactive use it is always used when
  present (A4); A9 warns that this means API charges instead of plan usage.
- Cloud-provider user: billed by that provider under the user's agreement with it (A1).

### 3. The adapter

See "The adapters" below: agentclientprotocol/claude-agent-acp, maintained by Zed Industries under the ACP GitHub
organization, Apache-2.0, built on the official Claude Agent SDK, reuses the Claude CLI's existing credential, not
described as official by Anthropic.

### 4. What the terms require of a third-party application

- Do not offer claude.ai login; do not collect, store or intermediate claude.ai credentials or session tokens; sign-in
  happens only in Anthropic's own flow (A1, A2).
- Do not route requests through Free, Pro, Max or Ultra credentials on behalf of users, or route third-party traffic
  against subscription limits (A1, A6, A14).
- Do not misrepresent the application's identity to Anthropic's servers (A6, A14).
- Do not pay for, resell or intermediate usage (A1).
- Names (A1): a product may say accurately, in plain text, that it runs Claude Code, but may not use the Claude Code or
  Anthropic names or logos in its own product, feature or company name or logo, or suggest Anthropic built, endorses or
  partners with it.
- Names for Agent SDK integrations (A2): "Claude Agent" is preferred in a dropdown, "Claude" inside a menu already
  labelled Agents, or "<name> Powered by Claude"; "Claude Code" and "Claude Code Agent" are not permitted, nor ASCII art
  or visuals that mimic Claude Code; the product keeps its own branding and must not appear to be an Anthropic product.
- A product that runs Claude Code itself takes on the A1 conditions (Commercial Terms, unmodified binary, no restricted
  sign-in methods).

## Findings for Codex

### 1. Third-party applications driving the user's own CLI or its SDK

- Sign-in methods (O1). Codex offers sign-in with ChatGPT (subscription access) or with an API key (usage-based). The CLI
  and IDE extension share one cached login, stored in a local file or the OS credential store, which OpenAI says to
  treat like a password. Administrators can force one method.
- App server (O2). OpenAI describes it as the interface Codex uses for rich clients and recommends it for a deep
  integration in your own product, authentication included. Its auth section says a local or open-source application
  built on Codex app-server authentication may continue to use it, recommends migrating to Sign in with ChatGPT so users
  get more control over and visibility into their usage, and states: "App-server authentication has never been permitted
  for commercial or hosted services." (https://learn.chatgpt.com/docs/app-server, read 2026-11-16). Clients should set
  `clientInfo` to identify their integration; its name identifies the client in OpenAI's compliance logs, and a new
  integration aimed at enterprises should ask OpenAI to add it to a known-clients list.
- Codex SDK (O3). Lists integrating Codex into your own application as a use, and points custom clients to the app
  server.
- Sign in with ChatGPT (O6 to O11, O17). This is OpenAI's sanctioned route for a third-party app to use a ChatGPT plan.
  Open-source and locally hosted apps register themselves and ask the user for a plan-usage permission; plan usage is
  open to all open-source partners and selected private clients, and covers eligible Plus and Pro users (O6, O7). Paid
  or remotely hosted apps go through an interest form (O7); commercial sign-in is waitlisted (O11). An app can drive
  Codex app-server with its own plan-usage token and identifies itself in `clientInfo` (O8), within preview limits (O9).
  The Sign in with ChatGPT Terms (O17, effective 2026-11-04) require the app to use its own name, never impersonate
  another app, obtain tokens only through OpenAI's flow, store them only locally under the user's control, never charge
  for plan use, send requests only from the user's own runtime, use the plan only for that app, and never pool, share or
  rotate accounts. O10 asks for a first-use confirmation, an in-use indicator and a link to ChatGPT usage settings.
- Terms of Use (O13, effective 2026-01-01). No sharing of account credentials; no circumventing rate limits or
  restrictions; no automatic or programmatic extraction of data or output; OpenAI names and logos only under the brand
  guidelines. A third-party client driving Codex: not stated in the sources read.
- Service Terms (O14, updated 2026-11-13). Codex output may be subject to third-party licences; apps that use Sign in
  with ChatGPT are bound by O17. A third-party client driving Codex with Codex's own login: not stated in the sources
  read. Usage Policies (O15): nothing on point found.
- Help Center (O12). Codex is included across ChatGPT plans, Free and Go included; users sign in with ChatGPT in an
  OpenAI client; the ChatGPT terms apply to data shared between Codex and ChatGPT. Third-party clients: not stated in the
  sources read.
- ACP and the adapter: not stated in the sources read. No OpenAI page read mentions ACP, Zed or codex-acp, and OpenAI's
  list of its open-source Codex components does not include an ACP adapter (O5).

Reading. API-key use through a third-party client: permitted (O1, O3, O4). Use of the user's own ChatGPT sign-in, held by
Codex and used through the app server, from a local or open-source application: permitted with limits (O2): never in a
commercial or hosted service, and OpenAI recommends moving to Sign in with ChatGPT. Doubt: O2 addresses applications
already built on app-server authentication that keep using it; whether a new application may start doing so is not
stated in the sources read. The codex-acp adapter drives the app server and reuses Codex's sign-in (R2), so O2 is the closest page to the
UniDeX arrangement, but O2 does not mention adapters.

### 2. Billing pool

- Subscription user signed in with ChatGPT: Codex use counts against the plan's usage limits; Codex shares one allowance
  and credit pool with ChatGPT Work and other agentic features; Plus and Pro users can buy credits after reaching a limit;
  Business, Edu and Enterprise use workspace credits or, on token-based Enterprise agreements, USD billing (O4, O12).
  Whether use through a third-party client draws on the same pool is not stated in the sources read; the docs tie the
  pool to the sign-in method, not the client. Under Sign in with ChatGPT the pool is the user's plan or available credits
  (O6).
- API-key user: billed through the user's OpenAI Platform account at standard API rates, not from plan credits (O1, O4).

### 3. The adapter

See "The adapters" below: agentclientprotocol/codex-acp, under the ACP GitHub organization with a JetBrains copyright
notice, Apache-2.0, built on the Codex App Server, reuses Codex's existing sign-in, not described as official by OpenAI.

### 4. What the terms require of a third-party application

- When relying on app-server authentication: stay a local or open-source application, never a commercial or hosted
  service (O2).
- Identify the integration in `clientInfo` (O2, O8).
- Do not share credentials or circumvent rate limits (O13).
- Do not misrepresent the relationship with OpenAI or imply endorsement; do not put OpenAI logos into the app's own
  branding (O16, O17).
- Further attribution requirements: not stated in the sources read.
- If an app adopts Sign in with ChatGPT instead, O17 and O10 apply in full.

## The adapters

| | Claude adapter | Codex adapter |
|---|---|---|
| Repository | https://github.com/agentclientprotocol/claude-agent-acp (npm `@agentclientprotocol/claude-agent-acp`, version 0.107.3 in package.json on the read date). The earlier address zed-industries/claude-code-acp redirects here. | https://github.com/agentclientprotocol/codex-acp (npm `@agentclientprotocol/codex-acp`, version 2.7.0 in package.json on the read date). Replaces zed-industries/codex-acp, which is archived and says development moved here (R3). |
| Maintainer | Zed Industries: package author is Zed Industries and the licence notice names Zed Industries, Inc. and contributors. Hosted under the ACP GitHub organization. Category: the editor company that created ACP, not the vendor. | Package author is "Agent Client Protocol"; the licence notice names JetBrains s.r.o. Hosted under the ACP GitHub organization. Category: the ACP project (governed by Zed and JetBrains, R5), not the vendor. |
| Licence | Apache-2.0 (LICENSE file and package.json). | Apache-2.0 (LICENSE file text and package.json; GitHub's metadata shows the licence as unrecognised). |
| How it runs the agent | Built on the Claude Agent SDK (depends on `@anthropic-ai/claude-agent-sdk`), which runs the Claude Code binary. Release archives ship the Claude CLI next to the adapter; `CLAUDE_CODE_EXECUTABLE` selects an installed CLI instead. | Starts the Codex App Server and translates ACP to it. The npm package depends on `@openai/codex`, so it brings its own Codex binary; `CODEX_PATH` selects an installed one instead. |
| Authentication | Uses whatever credential the Claude CLI already holds (subscription login, Console login, API key, key helper, cloud provider). Advertises two terminal login methods that run the CLI's own login: claude.ai subscription and Anthropic Console. A `--hide-claude-auth` flag hides the subscription method and refuses any session or turn that a Free, Pro, Max or Team plan would pay for; it still lets a Claude Enterprise plan through, citing an Anthropic allowance that no Anthropic page read here states. | Reuses the account Codex already holds; it asks for authentication only when Codex needs OpenAI auth and has no account. Advertises API key (`CODEX_API_KEY` or `OPENAI_API_KEY`), ChatGPT browser login (hidden when `NO_BROWSER` is set), ChatGPT device code (offered only to clients that support URL elicitation) and an opt-in custom gateway. Passes the ACP client's name, title and version to the app server as its `clientInfo`. |
| Described as official | No. Its README calls the Agent SDK official, not itself; no Anthropic page read mentions it (A5). The ACP site lists it as Zed's SDK adapter (R4). | No. No OpenAI page read mentions it (O5). The ACP site lists it as ACP's adapter (R4). |

The protocol itself is jointly governed by Zed and JetBrains, who intend to move it to an independent foundation (R5).
ACP lets an agent handle login itself or have the client run a terminal login (R6).

## Conclusion

- **Claude Code: ship with a stated limit.** The limit: the add-on runs only on API-key-class credentials, meaning an
  Anthropic API key, an Anthropic Console sign-in, or a supported cloud-provider credential. claude.ai subscription
  credentials, on any plan, are not supported through UniDeX. Basis: A1 and A6 (with A2): third-party tools and
  Agent-SDK-based products are directed to API keys, and offering a plan's limits needs Anthropic's prior approval. The
  doubt that stops a plain "ship": A1's carve-out for an end user signing in to the unmodified binary, and A7 and A10
  describing third-party use of plans, leave open a reading under which subscription use would be allowed. The sources
  do not settle it, so the conservative limit applies.
- **Codex: ship with a stated limit.** The limit: API-key use is always allowed. ChatGPT-plan use is allowed only through
  the user's own existing Codex sign-in, only while UniDeX is a free, locally run, open-source application, never as a
  commercial or hosted service; UniDeX never starts a ChatGPT login; and the add-on screen shows this use as permitted
  by OpenAI for local or open-source apps, with OpenAI's recommendation to move to Sign in with ChatGPT and a link to
  O2. Basis: O2 and O1. The doubt: O2's wording addresses apps that continue an existing integration, not new ones.

## What UniDeX must and must not do as a result

Both add-ons:

1. Never show a vendor login screen, never start a vendor login, and never ask for, read, copy, store or forward a
   claude.ai or ChatGPT credential, token or session (A1, O13). To sign in, the settings screen tells the user to run the
   vendor's own CLI login in their own terminal and links the vendor's instructions.
2. Never pay for, resell, pool or intermediate usage, and never let one person's sign-in serve another person's
   requests. Add-on sessions run on the user's own machine for that user only (A1, A14, O2, O13).
3. Identify UniDeX honestly: send UniDeX's own name, title and version as the ACP `clientInfo`, which codex-acp forwards
   to Codex (O2, R2); never present UniDeX as another client (A6, A14).
4. Names and branding: describe each add-on accurately in plain text, show no vendor logos, and imply no endorsement
   (A1, A2, O16). Label the Claude add-on "Claude Agent" in pickers; its settings text may say it runs the user's
   installed Claude Code through the Claude Agent ACP adapter. Label the Codex add-on "Codex"; its settings text may say
   it runs the user's installed Codex through the codex-acp adapter.
5. Point each adapter at the user's installed CLI (`CLAUDE_CODE_EXECUTABLE`, `CODEX_PATH`). This follows spec section
   5.4 (the user's own installed agent); it is not a vendor requirement.
6. The add-on settings screen shows which credential the agent is using, as the adapter reports it, which billing pool
   that means, and links to the vendor pages this record cites (A1, A6, O1, O2). It says the summary is UniDeX's reading,
   not the vendor's wording.
7. The ACP engine ships regardless of either add-on (spec section 5.4).

Claude add-on:

8. Spawn claude-agent-acp with `--hide-claude-auth`, so the subscription login is never offered and subscription-billed
   turns are refused (R1).
9. Treat any session the adapter reports as billed to a claude.ai plan as unsupported, Enterprise included, until the
   maintainer decides otherwise (open point 2).
10. UniDeX may pass an Anthropic API key that the user entered for this add-on, only in the adapter process's
    environment; usage is billed to the key owner (A1, A4).
11. Settings text, in substance: uses your Anthropic API key, Console sign-in or cloud provider; billed at API rates by
    Anthropic or your provider, not from a Claude plan; Claude plan sign-ins are not supported in UniDeX.

Codex add-on:

12. Spawn codex-acp with `NO_BROWSER=1`, and never show or call its ChatGPT or device-code login methods (R2).
13. UniDeX may pass an OpenAI API key that the user entered for this add-on, only in the adapter process's environment
    (`CODEX_API_KEY`); usage is billed to the user's OpenAI Platform account at API rates (O1).
14. If Codex already holds a ChatGPT sign-in, the add-on may use it. The settings screen then says, in substance: this
    uses your ChatGPT plan's Codex limits and credits; OpenAI permits local or open-source apps to use Codex's own
    sign-in but not commercial or hosted services, and recommends Sign in with ChatGPT; with a link to O2. The first such
    session shows a one-time confirmation.
15. The ChatGPT-plan path exists only in free, locally run, open-source builds. If UniDeX is ever sold, included in a
    paid offering, or run as a hosted service, that path is removed or replaced with Sign in with ChatGPT under O17, and
    this record is revisited.

## Open points for the maintainer

1. Claude subscriptions. If subscription use through the add-on matters (goal G4 and spec section 3), ask Anthropic
   directly: A1 points questions about permitted authentication to its sales contact, and A2 allows exceptions only with
   prior approval. The reading that might allow it (A1's carve-out for a platform running the unmodified binary) also
   carries conditions that fit an open-source desktop app poorly: the platform accepts the Commercial Terms and may not
   restrict built-in sign-in methods, which `--hide-claude-auth` does. Until Anthropic answers, G4 holds for Codex but
   not for Claude subscriptions.
2. The adapter's Enterprise exception. `--hide-claude-auth` lets a Claude Enterprise plan pay for turns, citing an
   Anthropic allowance that no page read here states. Default in this record: refuse it too. Accept the exception only
   with an Anthropic source.
3. Codex for a new app. O2 says existing local or open-source apps may continue. Options: accept that as covering UniDeX
   (this record's default, with the stated limit); ask OpenAI; or move to Sign in with ChatGPT. That last option means
   UniDeX would own an OAuth sign-in and store tokens, which rule 1 above and spec section 5.4 rule out, and its plan
   usage covers eligible Plus and Pro users only (O6).
4. Spec wording. Spec section 3 and goal G4 say subscriptions are reached through add-on engines. When records 0006 to
   0014 are folded into the spec, that line needs the limits above.
5. Bundled vendor binaries. Both adapters can bring their own copy of the vendor CLI (R1, R2). Rule 5 points them at the
   user's installed CLI; whether the bundled copies raise any vendor-terms issue is not stated in the sources read.
6. OpenAI Usage Policies. O15 showed a version effective 2026-11-20, after the read date; the version in force on the
   read date was not read. Re-read after 2026-11-20.

## Revisit when

- Before milestone M5 is planned (required).
- Whenever a cited page changes, above all A1, A2, A6, A7, A14, O2, O4 and O17. Both vendors changed their rules on
  third-party use of subscriptions several times in 2026, so re-read the cited pages before each release that ships an
  add-on.
- When Anthropic or OpenAI answers open point 1, 2 or 3.
- When either adapter changes maintainer, licence or authentication behaviour (for example the meaning of
  `--hide-claude-auth` or `NO_BROWSER`).
- When UniDeX's distribution changes: a paid edition, a hosted service, or bundling with another product.
