# Security Requirements

## Secrets

- Never commit API keys, tokens, passwords, or private keys.
- Use environment variables or an approved secret manager.
- Keep local secret files out of version control.
- Redact sensitive information from logs.

## Input and Data

- Validate untrusted input.
- Use parameterized database queries.
- Apply least privilege.
- Avoid unnecessary collection of personal data.
- Define data retention and deletion behavior.

## Dependencies

- Review maintenance status.
- Review licenses.
- Check known vulnerabilities.
- Avoid blindly executing untrusted scripts.

## Security Changes

Do not disable security controls to bypass problems.
Obtain approval before materially reducing security.
