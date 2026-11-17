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
record says it is not stated in the sources read, or not confirmed, and no permission is inferred from silence. Exactly
one short direct quotation is used (in the Codex findings); everything else is paraphrase.

This is an engineering record of what the cited pages say on the date read; it is not legal advice.

## Sources read

All dates read: 2026-11-16. IDs are used in the rest of this record. The Claude Help Center pages show only a relative
update date, given here as the page showed it.

| ID | Source | Address | Date read | What it was used for |
|---|---|---|---|---|
| A1 | Claude Code docs: Legal and compliance | https://code.claude.com/docs/en/legal-and-compliance | 2026-11-16 | Which terms govern Claude Code; authentication and credential rules for third-party developers; conditions for running Claude Code in a product; name and logo rules |
| A2 | Claude Code docs: Agent SDK overview | https://code.claude.com/docs/en/agent-sdk/overview | 2026-11-16 | No claude.ai login or rate limits in third-party products; branding guidelines; Commercial Terms govern the SDK |
| A3 | Claude Code docs: Agent SDK quickstart | https://code.claude.com/docs/en/agent-sdk/quickstart | 2026-11-16 | Supported SDK credentials (API key, cloud providers); repeats the A2 notice |
| A4 | Claude Code docs: Authentication | https://code.claude.com/docs/en/authentication | 2026-11-16 | Credential precedence; keyless Console sign-in; the long-lived OAuth token authenticates with the Claude subscription; settings files can set it |
| A5 | Claude Code docs index | https://code.claude.com/docs/llms.txt | 2026-11-16 | Searched for ACP, Agent Client Protocol and Zed: no page found |
| A6 | Claude Help Center: Log in to your Claude account (page showed: updated over a week ago) | https://support.claude.com/en/articles/13189465-log-in-to-your-claude-account | 2026-11-16 | Subscription use by third-party tools, including open-source projects; usage credits; prohibited tool behaviour |
| A7 | Claude Help Center: Use the Claude Agent SDK with your Claude plan (page showed: updated over 2 weeks ago) | https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan | 2026-11-16 | Billing pool for Agent SDK, `claude -p` and third-party app use on a plan |
| A8 | Claude Help Center: Monthly API credits for Max, Ultra, and Team plans (page showed: updated over 2 weeks ago) | https://support.claude.com/en/articles/17154008-monthly-api-credits-for-max-ultra-and-team-plans | 2026-11-16 | Which pool Agent SDK use draws on with a plan sign-in versus an API key; the June credit pool is not available |
| A9 | Claude Help Center: Use Claude Code with your Pro, Max, or Ultra plan (page showed: updated over 2 weeks ago) | https://support.claude.com/en/articles/11145838-use-claude-code-with-your-pro-max-or-ultra-plan | 2026-11-16 | Plan limits; an API key in the environment bills API rates instead of the plan |
| A10 | Claude Help Center: Buy usage bundles (page showed: updated over a week ago) | https://support.claude.com/en/articles/14246112-buy-usage-bundles | 2026-11-16 | Bundle balance also covers third-party products that use a Claude account |
| A11 | Claude Help Center: Manage usage credits for paid Claude plans (page showed: updated this week) | https://support.claude.com/en/articles/12429409-manage-extra-usage-for-paid-claude-plans | 2026-11-16 | What usage credits are (pay as you go after plan limits) |
| A12 | Anthropic Consumer Terms of Service (effective 2025-10-08) | https://www.anthropic.com/legal/consumer-terms | 2026-11-16 | Automated access only by API key or explicit permission; no sharing of credentials |
| A13 | Anthropic Commercial Terms of Service (effective 2025-06-17) | https://www.anthropic.com/legal/commercial-terms | 2026-11-16 | Govern Anthropic API keys and offerings that reference them; resale restriction; mentions neither Claude Code nor the Agent SDK |
| A14 | Anthropic Usage Policy (effective 2026-11-12) | https://www.anthropic.com/legal/aup | 2026-11-16 | Prohibits services that route requests through consumer subscriptions or misrepresent the client |
| A15 | Claude Code docs: Environment variables | https://code.claude.com/docs/en/env-vars | 2026-11-16 | `CLAUDE_CODE_OAUTH_TOKEN` and `CLAUDE_CODE_OAUTH_REFRESH_TOKEN` are claude.ai credentials; no file-descriptor variant is listed |
| O1 | Codex docs: Authentication (developers.openai.com/codex/auth redirects here) | https://learn.chatgpt.com/docs/auth | 2026-11-16 | ChatGPT sign-in versus API key; API key billing; the CLI's API-key login; shared login cache; credential store modes |
| O2 | Codex docs: Codex App Server | https://learn.chatgpt.com/docs/app-server | 2026-11-16 | Policy on local, open-source and commercial apps using app-server authentication; API-key login is stored by Codex; client identification |
| O3 | Codex docs: Codex SDK | https://learn.chatgpt.com/docs/codex-sdk | 2026-11-16 | Integrating Codex into your own application; custom clients pointed to the app server |
| O4 | Codex docs: Pricing | https://learn.chatgpt.com/docs/pricing | 2026-11-16 | Plan usage, credits and API-key billing for Codex |
| O5 | Codex docs: Open Source, and the Codex docs index | https://learn.chatgpt.com/docs/open-source and https://developers.openai.com/codex/llms.txt | 2026-11-16 | OpenAI's list of its open-source Codex components; searched for ACP and Zed: none found |
| O6 | Sign in with ChatGPT docs: index and Quickstart | https://developers.openai.com/siwc and https://developers.openai.com/siwc/quickstart | 2026-11-16 | OpenAI's sanctioned route for third-party apps to use a ChatGPT plan; who it is open to |
| O7 | Sign in with ChatGPT: ChatGPT plan usage overview | https://developers.openai.com/siwc/token-sharing-open-source | 2026-11-16 | Plan usage for open-source and locally hosted apps; host IDs; paid or remotely hosted apps need the interest form |
| O8 | Sign in with ChatGPT: Codex app-server | https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server | 2026-11-16 | How an app drives Codex app-server with its own plan-usage token; client identification |
| O9 | Sign in with ChatGPT: Preview limitations | https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations | 2026-11-16 | Limits of plan usage through Codex app-server |
| O10 | Sign in with ChatGPT: UI/UX guidelines | https://developers.openai.com/siwc/ui-ux-guidelines | 2026-11-16 | How OpenAI wants plan use shown to users |
| O11 | Sign in with ChatGPT: Request a client ID | https://developers.openai.com/siwc/request-client-id | 2026-11-16 | Commercial sign-in is waitlisted |
| O12 | OpenAI Help Center: Using Codex with your ChatGPT plan (page said it was updated 12 days before the read date) | https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan | 2026-11-16 | Plan availability, shared allowance and credit pool, which terms apply |
| O13 | OpenAI Terms of Use (effective 2026-01-01) | https://openai.com/policies/terms-of-use/ | 2026-11-16 | No sharing credentials; no circumventing rate limits; name and logo use |
| O14 | OpenAI Service Terms (updated 2026-11-13) | https://openai.com/policies/service-terms/ | 2026-11-16 | Codex section; Sign in with ChatGPT section |
| O15 | OpenAI Usage Policies, version effective 2026-11-20 (the current page) | https://openai.com/policies/usage-policies/ | 2026-11-16 | New clause on providing access to OpenAI models (model-access intermediaries) |
| O16 | OpenAI Design (brand) Guidelines | https://openai.com/brand/ | 2026-11-16 | No implied endorsement; no OpenAI logo in own branding |
| O17 | OpenAI Sign in with ChatGPT Terms (effective 2026-11-04) | https://openai.com/policies/sign-in-with-chatgpt-terms/ | 2026-11-16 | Obligations an app takes on if it uses Sign in with ChatGPT plan usage (open point only) |
| O18 | OpenAI Usage Policies, version effective 2025-10-29 (in force on the read date, linked from O15) | https://openai.com/policies/usage-policies/revisions/2025-10-29/ | 2026-11-16 | Checked for a clause on model access or intermediaries: none |
| R1 | Repository agentclientprotocol/claude-agent-acp: README, LICENSE, package.json, `src/auth-methods.ts`, `src/hide-claude-auth.ts`, `src/auth-status.ts`, `src/v1/acp-agent.ts`, `src/v2/subscription-guard.ts`, `src/v2/auth.ts`, `src/tests/v1/authorization.test.ts`, repository metadata (guard details read at commit 0073280) | https://github.com/agentclientprotocol/claude-agent-acp | 2026-11-16 | Claude adapter: maintainer, licence, how it runs Claude Code, authentication, the subscription guard flag and its gaps, auth status reporting |
| R2 | Repository agentclientprotocol/codex-acp: README, LICENSE, package.json, `src/CodexAuthMethod.ts`, `src/CodexAcpClient.ts`, `src/CodexAccountLogin.ts`, `src/AuthStatusMeta.ts`, repository metadata (auth details read at commit 7c98a16) | https://github.com/agentclientprotocol/codex-acp | 2026-11-16 | Codex adapter: maintainer, licence, how it runs Codex, authentication, auth status reporting, client identity |
| R3 | Repository zed-industries/codex-acp (archived): README, LICENSE | https://github.com/zed-industries/codex-acp | 2026-11-16 | Earlier Codex adapter; points to R2 |
| R4 | Agent Client Protocol: Agents | https://agentclientprotocol.com/get-started/agents | 2026-11-16 | How the protocol's site lists the two adapters |
| R5 | Agent Client Protocol: Governance | https://agentclientprotocol.com/community/governance | 2026-11-16 | ACP is jointly governed by Zed and JetBrains |
| R6 | Agent Client Protocol: Authentication (v1) | https://agentclientprotocol.com/protocol/v1/authentication | 2026-11-16 | Agent-handled versus terminal login in ACP |

Pages not reached or not read: the openai.com policy pages (O13 to O18) refused a plain HTTP fetch (status 403) and were
read in a full browser instead, so they were reached. Not read: Anthropic's Trademark Guidelines (A1 summarises the
relevant rule).

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
- Running Claude Code inside a product (A1). Preinstalling or running Claude Code in a product or service, with hosted
  sandboxes and other agent infrastructure as the examples, requires agreeing to the Commercial Terms. The binary must be
  installed and run as published, and no built-in authentication method may be removed, disabled or restricted
  (signing in with a Claude account and with the user's own API key are named). The product may not pay for, resell or intermediate usage.
  Each end user authenticates with their own Anthropic API key, their own Claude subscription plan credentials, or a
  third-party inference provider credential, and is billed directly.
- Which A1 section applies to UniDeX: open. UniDeX starts an adapter that runs the user's own installed Claude Code on
  the user's machine. The sources read do not say whether that is running Claude Code in a product (the section above)
  or a third-party product built on the Agent SDK (the developer rules above and A2). If it is the first, A1 lets end
  users use their Claude plan credentials and forbids restricting built-in sign-in methods, so the limit chosen below,
  which refuses plan sign-ins and uses the adapter's flag that hides the claude.ai sign-in, may itself conflict with A1.
  If it is the second, A1 and A2 direct the product to API keys. This question goes to Anthropic (open point 1).
- Agent SDK pages (A2, A3). Unless Anthropic has approved it beforehand, third-party developers may not offer claude.ai
  login or rate limits in their products, agents built on the Agent SDK included; they should use the API-key methods in
  the quickstart (Anthropic API key, or Amazon Bedrock, Claude Platform on AWS, Google Cloud's Agent Platform, Microsoft
  Foundry).
- Help Center (A6). Plan usage is designed for ordinary use of Anthropic's own applications. The preferred route for
  third-party tools, open-source projects included, is API-key authentication through Claude Console or a supported
  cloud provider. Anthropic may, at its discretion, let paid subscribers who have turned on usage credits use certain
  third-party tools, and may bill that use to usage credits instead of plan limits. Tools that misrepresent their
  identity to Anthropic's servers or try to route third-party traffic against subscription limits are prohibited.
  Anyone building a tool for others is told to use API keys.
- Usage Policy (A14, effective 2026-11-12). Reselling, proxying or otherwise providing access to Claude through
  unauthorized means is prohibited, naming services that route requests through consumer subscriptions or misrepresent
  the product or client in use.
- Consumer Terms (A12). Automated or non-human access is not allowed except by an Anthropic API key or where Anthropic
  explicitly permits it; account credentials may not be shared.
- Long-lived OAuth token (A4, A15). A token made with the CLI's setup-token command and set as
  `CLAUDE_CODE_OAUTH_TOKEN` authenticates with the user's Claude subscription and needs a Pro, Max, Ultra, Team or
  Enterprise plan (A4). It outranks the stored `/login` credential but ranks below an API key in the precedence list
  (A4). Besides the shell, user settings files can set it (A4), and managed settings can too; project and local
  settings cannot (A15). A15 also lists `CLAUDE_CODE_OAUTH_REFRESH_TOKEN` as a claude.ai credential. A file-descriptor
  variant appears in R1's code but is not documented in the Anthropic pages read.
- Statements pointing the other way. A7 (its June 2026 update, still on the page) says Agent SDK, `claude -p` and
  third-party app usage still draw from subscription limits. A10 says the usage-bundle balance also covers third-party
  products that use a Claude account. A1 says advertised plan limits assume ordinary, individual use of Claude Code and
  the Agent SDK.
- ACP and the adapter: not stated in the sources read. No Anthropic page read mentions ACP, Zed or the Claude ACP
  adapter (A5). Which third-party tools Anthropic allows on a plan under A6: not stated in the sources read.

Reading. API-key and cloud-provider credentials through a third-party tool: A1, A2, A3 and A6 direct developers and
third-party tools to API keys through Claude Console or a supported cloud provider; the other Anthropic pages read are
silent on it. In this record that means a Console API key, including the key a Console login creates (A4), or a cloud
provider credential. A keyless Console sign-in (A4: an OAuth login kept as a profile, with no API key) is not mentioned
by those pages: whether it counts as API-key authentication is not stated in the sources read. claude.ai subscription
credentials through a third-party tool built on the Agent SDK: not clearly permitted. The most specific pages (A1, A2,
A6) direct such tools to API keys and say a plan's limits may not be offered without approval; A14 adds a prohibition
on routing through consumer subscriptions. The route that might cover it is A1's section on running Claude Code in a
product, and whether UniDeX falls under it is open (above).

### 2. Billing pool

- Subscription user signed in with a plan, no API key present: Agent SDK and `claude -p` use draws from the plan's usage
  limits (A7, A8). A6 reserves Anthropic's right to bill third-party tool use to usage credits instead (pay as you go at
  API rates, A11); which tools that applies to is not stated in the sources read. The Max, Ultra and Team monthly API
  credits do not cover plan-signed-in use (A8). The separate Agent SDK credit announced in June is not available (A8).
- API-key user: billed to the key owner at standard API rates (A1, A9). The Max, Ultra and Team monthly API credits do
  cover the Agent SDK and `claude -p` when run with an API key from the Console organization linked to the plan (A8). An
  `ANTHROPIC_API_KEY` in the environment outranks a subscription login, and in non-interactive use it is always used when
  present (A4, A15); A9 warns that this means API charges instead of plan usage.
- Cloud-provider user: billed by that provider under the user's agreement with it (A1).

### 3. The adapter

See The adapters, below: agentclientprotocol/claude-agent-acp, maintained by Zed Industries (one of the two companies
that jointly govern ACP, R5) under the ACP GitHub organization, Apache-2.0, built on the official Claude Agent SDK,
reuses the Claude CLI's existing credential, not described as official by Anthropic. Its subscription guard flag is a
partial backstop, not a guarantee.

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
- Names for Agent SDK integrations (A2): the name Claude Agent is preferred in dropdown menus; the bare name Claude is
  allowed inside a menu already labelled for agents; a product with its own agent name may add a powered-by-Claude
  tagline. Naming the integration Claude Code or Claude Code Agent is not permitted, nor are ASCII art or visuals that
  mimic Claude Code; the product keeps its own branding and must not appear to be an Anthropic product.
- If UniDeX counts as a product running Claude Code, it takes on the A1 conditions (Commercial Terms, unmodified
  binary, no restricted sign-in methods); whether it counts is open.

## Findings for Codex

### 1. Third-party applications driving the user's own CLI or its SDK

- Sign-in methods (O1). Codex offers sign-in with ChatGPT (subscription access) or with an API key (usage-based); the
  CLI takes an API key through its login command reading the key from standard input. The CLI and IDE extension share
  one cached login, stored in a local file or the OS credential store, which OpenAI says to treat like a password. A
  setting chooses the store: a file under the Codex home folder, the OS credential store, either automatically, or an
  in-memory store for the current process only. Administrators can force one login method.
- App server (O2). OpenAI describes it as the interface Codex uses for rich clients and recommends it for a deep
  integration in your own product, authentication included. In API-key mode the caller supplies a key and Codex stores
  it for API requests. Its auth section says a local or open-source application already built on Codex app-server
  authentication may keep using it, recommends migrating to Sign in with ChatGPT so users get more control over and
  visibility into their usage, and states: "App-server authentication has never been permitted for commercial or
  hosted services." (https://learn.chatgpt.com/docs/app-server, read 2026-11-16). On its face that sentence covers
  app-server authentication in every mode, API-key mode included. Clients should set `clientInfo` to identify their
  integration; its name identifies the client in OpenAI's compliance logs, and a new integration aimed at enterprises
  should ask OpenAI to add it to a known-clients list.
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
- Usage Policies (O15, O18). The current page is a version effective 2026-11-20, after the read date. It adds a clause
  on providing access to OpenAI models: whoever operates a model-access intermediary platform, or another service that
  lets third parties directly access OpenAI models, needs OpenAI's prior review and approval; the clause does not apply
  merely because an application integrates OpenAI models to power its own features (O15). The version in force on the
  read date, effective 2025-10-29, has no such clause (O18). This record's reading, not OpenAI's: the Codex add-on runs
  the user's own Codex for that user's own work inside UniDeX, which looks like an application integrating models to
  power its features rather than a service giving third parties direct model access. That reading would need checking
  again if UniDeX ever let other people or other tools reach a Codex-backed session, for example through UniDeX's own
  MCP server or its command-line interface used on someone else's behalf.
- Terms of Use (O13, effective 2026-01-01). No sharing of account credentials; no circumventing rate limits or
  restrictions; no automatic or programmatic extraction of data or output; OpenAI names and logos only under the brand
  guidelines. A third-party client driving Codex: not stated in the sources read.
- Service Terms (O14, updated 2026-11-13). Codex output may be subject to third-party licences; apps that use Sign in
  with ChatGPT are bound by O17. A third-party client driving Codex with Codex's own login: not stated in the sources
  read.
- Help Center (O12). Codex is included across ChatGPT plans, Free and Go included; users sign in with ChatGPT in an
  OpenAI client; the ChatGPT terms apply to data shared between Codex and ChatGPT. Third-party clients: not stated in the
  sources read.
- ACP and the adapter: not stated in the sources read. No OpenAI page read mentions ACP, Zed or codex-acp, and OpenAI's
  list of its open-source Codex components does not include an ACP adapter (O5).

Reading. API-key use from a local, non-commercial third-party client: O2 and O3 describe integrating Codex into your own
product or application, O1 and O4 describe API-key use billed at API rates, and none of the OpenAI pages read forbids it;
O2's sentence on commercial or hosted services applies to it on its face. Use of the user's own ChatGPT sign-in, held by
Codex and used through the app server: O2 addresses applications already built on app-server authentication that keep
using it, and recommends Sign in with ChatGPT; whether a new application may start doing so is not stated in the
sources read. The codex-acp adapter drives the app server and reuses Codex's sign-in (R2), so O2 is the closest page to
the UniDeX arrangement, but O2 does not mention adapters.

### 2. Billing pool

- Subscription user signed in with ChatGPT: Codex use counts against the plan's usage limits; Codex shares one allowance
  and credit pool with ChatGPT Work and other agentic features; Plus and Pro users can buy credits after reaching a limit;
  Business, Edu and Enterprise use workspace credits or, on token-based Enterprise agreements, USD billing (O4, O12).
  Whether use through a third-party client draws on the same pool is not stated in the sources read; the docs tie the
  pool to the sign-in method, not the client. Under Sign in with ChatGPT the pool is the user's plan or available credits
  (O6). Under this record's conclusion UniDeX does not use this pool.
- API-key user: billed through the user's OpenAI Platform account at standard API rates, not from plan credits (O1, O4).

### 3. The adapter

See The adapters, below: agentclientprotocol/codex-acp, under the ACP GitHub organization with a JetBrains copyright
notice, Apache-2.0, built on the Codex App Server, reuses Codex's existing sign-in, not described as official by OpenAI.

### 4. What the terms require of a third-party application

- When relying on app-server authentication, in any mode: never be a commercial or hosted service (O2).
- Identify the integration in `clientInfo` (O2, O8).
- Do not share credentials or circumvent rate limits (O13).
- From 2026-11-20: do not operate a model-access intermediary service without OpenAI's approval (O15).
- Do not misrepresent the relationship with OpenAI or imply endorsement; do not put OpenAI logos into the app's own
  branding (O16, O17).
- Further attribution requirements: not stated in the sources read.
- If an app adopts Sign in with ChatGPT instead, O17 and O10 apply in full.

## The adapters

| | Claude adapter | Codex adapter |
|---|---|---|
| Repository | https://github.com/agentclientprotocol/claude-agent-acp (npm `@agentclientprotocol/claude-agent-acp`, version 0.107.3 in package.json on the read date). The earlier address zed-industries/claude-code-acp redirects here. | https://github.com/agentclientprotocol/codex-acp (npm `@agentclientprotocol/codex-acp`, version 2.7.0 in package.json on the read date). Replaces zed-industries/codex-acp, which is archived and says development moved here (R3). |
| Maintainer | Zed Industries: package author is Zed Industries and the licence notice names Zed Industries, Inc. and contributors. Hosted under the ACP GitHub organization. Category: one of the two companies that jointly govern ACP (R5); not the vendor. | The package author field names the Agent Client Protocol project; the licence notice names JetBrains s.r.o. Hosted under the ACP GitHub organization. Category: the ACP project, governed jointly by Zed and JetBrains (R5); not the vendor. |
| Licence | Apache-2.0 (LICENSE file and package.json). | Apache-2.0 (LICENSE file text and package.json; GitHub's metadata shows the licence as unrecognised). |
| How it runs the agent | Built on the Claude Agent SDK (depends on `@anthropic-ai/claude-agent-sdk`), which runs the Claude Code binary. Release archives ship the Claude CLI next to the adapter; `CLAUDE_CODE_EXECUTABLE` selects an installed CLI instead. | Starts the Codex App Server and translates ACP to it. The npm package depends on `@openai/codex`, so it brings its own Codex binary; `CODEX_PATH` selects an installed one instead. |
| Authentication | Uses whatever credential the Claude CLI already holds (subscription login, Console login, API key, key helper, environment OAuth token, cloud provider). Advertises two terminal login methods that run the CLI's own login: claude.ai subscription and Anthropic Console. | Uses the account Codex already holds: it asks for a login only when Codex needs OpenAI auth and has no account (`readAuthRequirement`), so an API key placed only in the environment is ignored while a ChatGPT account is cached. Advertises API key (`CODEX_API_KEY` or `OPENAI_API_KEY`), ChatGPT browser login (hidden when `NO_BROWSER` is set), ChatGPT device code (offered only to clients that support URL elicitation) and an opt-in custom gateway. Choosing its API-key method makes it log in to the app server with an API key, which Codex stores (O2) in the login cache the CLI and IDE share (O1); whether O1's store modes change that for the app server the adapter starts is not verified. Passes the ACP client's name, title and version to the app server as its `clientInfo`. |
| Subscription guard | The `--hide-claude-auth` flag hides the subscription login method and refuses a session or turn when the account the SDK reports looks billed to any Claude plan other than Enterprise (not only the Free, Pro, Max and Team plans). Gaps, each confirmed in R1's code or tests: an account whose credential is an environment OAuth token passes the guard even with a Pro plan (the token-source test case), although A4 says that token authenticates with the Claude subscription; a Claude Enterprise plan passes, citing an Anthropic allowance that no Anthropic page read states; the guard fails open when the account cannot be read; a credential swapped mid-session is seen only at the next prompt; the guard is off while a provider or gateway override routes traffic; the adapter's `/remote-control` command is not guarded in either the v1 or the v2 code path. | None. Nothing in R2 refuses a ChatGPT-billed session. |
| Identity reporting | Reports the identity the agent uses through an `_auth/status_update` notification and the ACP `auth/status` request: kind `account` (a Claude plan), `api_key`, `gateway`, `external` (cloud provider) or `none`. When it cannot tell, it sends nothing and `auth/status` answers unknown. Reporting only. | Pushes the identity in an `_auth/status_update` notification after `initialize` and on each change: kind `account` for a ChatGPT sign-in, `api_key` for an API key and also for a personal access token, `gateway`, one of its external kinds, or `none`. When it cannot tell, it sends nothing. |
| Refusals | Answers a refused or signed-out session with the ACP auth-required error; a refused subscription carries a reason field naming that case. | Answers with the ACP auth-required error when Codex has no account and no default login request is configured; with `DEFAULT_AUTH_REQUEST` set, it logs in by itself instead. No refusal based on which account pays. |
| Described as official | No. Its README calls the Agent SDK official, not itself; no Anthropic page read mentions it (A5). The ACP site lists it as Zed's SDK adapter (R4). | No. No OpenAI page read mentions it (O5). The ACP site lists it as ACP's adapter (R4). |

The protocol itself is jointly governed by Zed and JetBrains, who intend to move it to an independent foundation (R5).
ACP lets an agent handle login itself or have the client run a terminal login (R6).

## Conclusion

Summary: both add-ons ship for API-key use only. Design goal G4 (subscriptions reached through add-on engines) holds for
neither agent until a vendor confirms otherwise (open points 1 and 3).

- **Claude Code: ship with a stated limit.** The limit: the add-on runs only on a Console API key (including the key a
  Console login creates) or a supported cloud-provider credential. Not supported: claude.ai subscription credentials on
  any plan (Enterprise included), environment OAuth tokens, and keyless Console sign-in (whether it counts as API-key
  authentication is not stated). UniDeX enforces the limit itself, with a cleaned environment and a check of the paying
  credential before each turn; the adapter's flag is only a backstop. Basis: A1 and A6, with A2: third-party tools and
  Agent-SDK-based products are directed to API keys, and offering a plan's limits needs Anthropic's prior approval. The
  doubt that stops a plain ship: A1's section on running Claude Code in a product, and A7 and A10 describing
  third-party use of plans, leave open a reading under which plan use would be allowed, and under that reading the
  limit itself may conflict with A1. The sources do not settle it.
- **Codex: ship with a stated limit.** The limit: API key only. Use of a ChatGPT plan through the user's existing Codex
  sign-in is not offered until OpenAI confirms that a new local, open-source client may use app-server authentication,
  or UniDeX is accepted for the Sign in with ChatGPT route (open point 3). Basis: O2 and O1. The doubt: O2 lets
  applications already built on app-server authentication continue, and does not say a new one may start.

## What UniDeX must and must not do as a result

Both add-ons:

1. Never show a vendor login screen, never call an adapter's login or logout methods, never start a vendor login, never
   ask for, read, copy, store or forward a claude.ai or ChatGPT credential, token or session, and never write to an
   agent's shared credential store (A1, O1, O13). Sign-in steps are shown as instructions the user follows in their own
   terminal.
2. Never pay for, resell, pool or intermediate usage, and never let one person's credential serve another person's
   requests. Add-on sessions run on the user's own machine for that user only (A1, A14, O2, O13, O15).
3. Identify UniDeX honestly: send UniDeX's own name, title and version as the ACP `clientInfo`, which codex-acp forwards
   to Codex (O2, R2); never present UniDeX as another client (A6, A14).
4. Names and branding: describe each add-on accurately in plain text, show no vendor logos, and imply no endorsement
   (A1, A2, O16). The Claude add-on uses the name form A2 prefers for dropdown menus (Claude Agent); its settings text
   may say it runs the user's installed Claude Code through the Claude ACP adapter. The Codex add-on is named Codex; its
   settings text may say it runs the user's installed Codex through the codex-acp adapter.
5. Point each adapter at the user's installed CLI (`CLAUDE_CODE_EXECUTABLE`, `CODEX_PATH`). This follows spec section
   5.4 (the user's own installed agent); it is not a vendor requirement.
6. Before each turn, UniDeX reads the identity the adapter last reported (both adapters push `_auth/status_update`; the
   Claude adapter also answers `auth/status`; R1, R2) and runs the turn only when that report shows a credential the
   add-on allows (rules 10 and 15). When nothing has been reported, or the report shows anything else, UniDeX refuses the
   turn and says why. The report is the adapter's reading, not proof, which is why rules 9 and 15 also control what
   credential the adapter can see. The add-on settings screen shows the reported credential, the billing pool it
   implies, and links to the vendor pages this record cites; it says the summary is UniDeX's reading, not the vendor's
   wording.
7. When an adapter answers with the ACP auth-required error (R1 uses it for a refused subscription, with a reason field,
   and for a signed-out CLI; R6), UniDeX shows which credential the add-on needs and how the user sets it up, and never
   calls the adapter's login method in response.
8. The ACP engine ships regardless of either add-on (spec section 5.4).

Claude add-on:

9. Spawn the adapter with a cleaned environment that carries only the credential the user chose for this add-on (an
   Anthropic API key, or a cloud provider's variables). It never carries `CLAUDE_CODE_OAUTH_TOKEN`, its file-descriptor
   variant (named in R1's code, not documented in the Anthropic pages read), or `CLAUDE_CODE_OAUTH_REFRESH_TOKEN`. A4
   says the first authenticates with the Claude subscription and R1's guard lets it through; A15 lists the refresh
   token as a claude.ai credential. A cleaned
   process environment does not cover a token set in Claude Code's own settings files (A4), which is why rule 10's check
   is still required. Whether rule 10 catches such a token when a Console key is also stored is not confirmed.
10. Run a turn only when the adapter reports kind `api_key` or `external`. For `api_key`, also check the report's detail
    field, because the adapter reports a gateway's bearer token under that kind too, and refuse a turn whose detail
    shows a gateway token. Refuse `account` (any Claude plan, Enterprise included), `gateway`, `none` and no report.
    Whether the adapter reports a keyless Console sign-in as `api_key` is not confirmed; until it is, the settings text
    says keyless Console sign-in is not supported, and this rule cannot enforce that exclusion: a keyless Console
    sign-in reported as `api_key` would pass.
11. Also spawn the adapter with `--hide-claude-auth`, as a backstop that hides its subscription login and refuses some
    plan-billed turns. It is never the guarantee; its gaps are listed under The adapters. UniDeX never sends the
    adapter's `/remote-control` command, which the flag does not guard in either code path.
12. UniDeX may pass an Anthropic API key that the user entered for this add-on, only in the adapter process's
    environment; usage is billed to the key owner (A1, A4).
13. Settings text, in substance: uses your Anthropic API key or cloud provider; billed at API rates by Anthropic or your
    provider, not from a Claude plan; Claude plan sign-ins, including long-lived tokens made from a plan, are not
    supported in UniDeX.

Codex add-on:

14. Spawn codex-acp with `NO_BROWSER=1` and without `DEFAULT_AUTH_REQUEST`; never show or call its ChatGPT,
    device-code or API-key login methods (rule 1).
15. Requirement: run a turn only when codex-acp reports kind `api_key` labelled as an OpenAI API key. Refuse `account`
    (a ChatGPT sign-in, on any plan), a personal access token (reported under the same kind, and which pool it bills is
    not established), `gateway`, `none` and no report. How the key reaches Codex is an open design question for the M5
    plan (open point 7). Two candidates, both within rule 1: the user configures Codex themselves, for example with the
    CLI's API-key login (O1), knowing that it replaces the ChatGPT sign-in their CLI and IDE share; or UniDeX gives the
    adapter a credential store isolated from the user's own Codex login, if one is confirmed. O1 documents an in-memory
    store mode and a file store under the Codex home folder; whether either can be applied to the app server that
    codex-acp starts, without touching the user's own login, is not verified. Setting `CODEX_API_KEY` alone has no
    effect while Codex holds a ChatGPT account (R2).
16. When codex-acp reports a ChatGPT sign-in, the settings screen says that ChatGPT-plan use is not offered in UniDeX for
    now. Anything it says about OpenAI's position stays within O2: OpenAI lets local or open-source applications already
    built on app-server authentication keep using it, says it was never permitted for commercial or hosted services, and
    recommends Sign in with ChatGPT; with a link to O2.
17. O2's sentence on commercial or hosted services covers app-server authentication on its face, API-key mode included.
    If UniDeX is ever sold, included in a paid offering, or run as a hosted service, the Codex add-on as a whole is
    suspended until this record is revisited; the same change triggers a check against O15's intermediary clause and
    A1's product conditions.

## Open points for the maintainer

1. Claude: questions for Anthropic. A1 points questions about permitted authentication to its sales contact, and A2
   allows exceptions only with prior approval. Ask: (a) whether a desktop app that starts an ACP adapter running the
   user's own installed Claude Code is a product running Claude Code under A1, or a third-party product built on the
   Agent SDK; (b) if the former, whether it may refuse claude.ai plan credentials, as this record's limit does, without
   breaching A1's ban on restricting built-in sign-in methods, and whether end users may use their plans through it;
   (c) if the latter, whether plan use could be approved under A2. Until Anthropic answers, the Claude add-on is
   API-key only.
2. The adapter's Enterprise exception. `--hide-claude-auth` lets a Claude Enterprise plan pay for turns, citing an
   Anthropic allowance that no page read here states. This record refuses it (rule 10). Accept the exception only with an
   Anthropic source.
3. Codex: the plan-backed option, as a decision for the maintainer. This record chooses option A; the maintainer can
   overrule it knowingly.
   - Option A, API key only (this record's default). Requires nothing more from OpenAI. Gives up ChatGPT-plan use for
     Codex in UniDeX. For: O2 does not say a new app may start using app-server authentication, and OpenAI's stated route
     for plan use by third-party apps is Sign in with ChatGPT (O6, O7). Against: no OpenAI page read forbids a local,
     open-source client outright.
   - Option B, allow the user's existing Codex sign-in. Requires OpenAI to confirm that a new local, open-source client
     may use app-server authentication, and UniDeX staying free, local and open source (rule 17). For: O2 lets local or
     open-source applications already built on it continue, and limits the ban to commercial or hosted services.
     Against: O2 speaks only of existing applications and recommends moving to Sign in with ChatGPT.
   - Option C, Sign in with ChatGPT. Requires UniDeX to register as an open-source client and keep a host ID per
     installation (O7), own an OAuth sign-in and store tokens locally under the user's control (O17), drive the app server
     with its own token (O8) within preview limits (O9), show plan use as O10 asks, and accept O17 (own name, no charge
     for plan use, plan used only for UniDeX, no pooling). This reverses rule 1 and the product rule stated under
     Context that UniDeX never handles a vendor login (spec section 5.4 says the app never bundles or installs an
     add-on, not that it never handles a login). For: it is OpenAI's recommended route, and plan usage is open to all
     open-source partners (O6). Against: a design change; plan usage covers eligible Plus and Pro users only (O6); a
     paid or remotely hosted UniDeX would need OpenAI's approval (O7).
4. Spec wording. Spec section 3 and goal G4 say subscriptions are reached through add-on engines. With these
   conclusions that holds for neither agent until a vendor confirms; when records 0006 to 0014 are folded into the spec,
   that line needs changing.
5. Bundled vendor binaries. Both adapters can bring their own copy of the vendor CLI (R1, R2). Rule 5 points them at the
   user's installed CLI; whether the bundled copies raise any vendor-terms issue is not stated in the sources read.
6. OpenAI Usage Policies. The intermediary clause takes effect on 2026-11-20 (O15). Re-read it then, and check any
   UniDeX feature that would let other people or tools reach a Codex-backed session against it.
7. M5 design questions. (a) How the Codex add-on gets an API key without UniDeX writing to the user's shared Codex
   login (rule 15): verify whether an in-memory store or a separate Codex home folder can be applied to the app server
   that codex-acp starts. (b) How UniDeX builds the cleaned environment for the Claude adapter (rule 9) and what it shows
   when a token set in Claude Code's own settings makes the adapter report a plan.

## Revisit when

- Before milestone M5 is planned (required).
- Whenever a cited page changes, above all A1, A2, A4, A6, A7, A14, O2, O4, O15 and O17. Both vendors changed their
  rules on third-party use of subscriptions several times in 2026, so re-read the cited pages before each release that
  ships an add-on.
- On or after 2026-11-20, when the new OpenAI Usage Policies take effect (open point 6).
- When Anthropic or OpenAI answers open point 1 or 3.
- When either adapter changes maintainer, licence, authentication or identity reporting (for example the meaning of
  `--hide-claude-auth`, `NO_BROWSER` or `_auth/status_update`).
- When UniDeX's distribution changes: a paid edition, a hosted service, or bundling with another product (rule 17).
