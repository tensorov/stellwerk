# Security Policy

## Supported branch

Only `main` is supported. Stellwerk is pre-1.0, so there are no maintained release branches and no backports. Fixes land on `main` first.

## Reporting a vulnerability

Report vulnerabilities through private GitHub security advisories for this repository. Use the "Report a vulnerability" button on the Security tab. Do not open a public issue for a security problem. If the advisory form is unavailable, contact the maintainer at aleksei.v.molchanov@gmail.com.

Include the affected revision, a reproduction, and the impact you observed. You should receive an acknowledgement once the report is triaged.

## Identity

The author of Stellwerk is **Aleksei Molchanov**, public handle **`tensorov`**. The repository is published under that handle.

## Composition of the commit history

**HEAD is cleaned.** Internal run artifacts are untracked and ignored, and live development endpoints have been redacted from the public plan files.

**The commit history is not cleaned, and no history rewrite was performed.** Эта история коммитов не очищалась и не перезаписывалась. The history contains, and should be expected to continue to contain:

- the author's name and addresses, including an old mail domain and a gmail address;
- early commits made under a псевдоним, alongside commits under the author's name and public handle;
- the old development host and SSH port;
- ablation results and model names used during development;
- `Co-authored-by: Sisyphus` trailers and other agent or model co-author trailers;
- internal journals, findings files, and run logs committed before the publication cleanup.

Do not treat the history as a sanitized artifact. Review it yourself before redistributing a checkout. The decisions behind the publication boundary live in `docs/adr/`; this file does not restate them.

## IP attestation

Код написан вне рамок найма, это личный open-source проект, права принадлежат автору.

In English: the code was written outside the scope of employment, this is a personal open-source project, and the rights belong to the author.
