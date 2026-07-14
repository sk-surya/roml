# Security Policy for ROML

## Supported Versions

ROML is in a **pre-1.0 development phase**. There are no stable releases yet. All versions are under active development and should not be considered production-ready.

| Version | Supported |
|---------|-----------|
| main    | Security fixes are applied to the main branch as part of normal development |
| < 1.0   | No formal security support; fixes are provided on a best-effort basis |

Once a 1.0.0 stable release is published, this policy will be updated with version-specific support windows.

## Reporting a Vulnerability

### Do NOT file a public GitHub issue for security vulnerabilities.

If you discover a security vulnerability in ROML, please report it through one of the following channels:

1. **GitHub Security Advisory**: Use the "Report a Vulnerability" feature on the [GitHub Security Advisories page](https://github.com/<org>/roml/security/advisories/new) for the repository (recommended).
2. **Email**: If the advisory feature is unavailable, contact the maintainers directly at the email address listed in the repository's `README.md` or crate metadata.

### What to include

- A clear description of the vulnerability
- Steps to reproduce (proof of concept is helpful)
- Affected versions and components
- Any potential mitigations you have identified

### Response timeline

- **Acknowledgment**: within 5 business days of receiving the report
- **Assessment and triage**: within 10 business days
- **Fix timeline**: depends on severity; critical vulnerabilities will be prioritized

We will coordinate disclosure and release timing with you. We ask that you do not publicly disclose the vulnerability until we have had a reasonable opportunity to address it.

## Scope

The following are in scope for the security policy:

- The `roml` core crate
- The `roml-highs`, `roml-mosek`, and `roml-xpress` adapter crates
- Build scripts and CI/CD configurations that affect package integrity
- Dependencies and transitive dependencies (where the vulnerability is exploitable through ROML)

## Out of Scope

The following are considered out of scope:

- Issues in third-party solvers (HiGHS, MOSEK, Xpress) themselves. Report those to the respective projects.
- Denial-of-service attacks that require local access or unusual system configurations.
- Hypothetical vulnerabilities without a demonstrated exploit path.
- Vulnerabilities in the Rust toolchain or standard library.

## Bug Bounty

ROML does not operate a bug bounty program. Vulnerability reports are accepted on a voluntary disclosure basis. We are grateful to security researchers who help improve the project and will acknowledge contributions (with the reporter's consent) in release notes or a security acknowledgments file.

## Contact

For security-related inquiries, please use the GitHub Security Advisory feature or the maintainer contact information published in the crate metadata.

## Policy Updates

This security policy may be updated as the project matures. Changes will be reflected in this file.
