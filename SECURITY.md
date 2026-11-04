# Security policy

## Supported versions

Security fixes go into the newest release. Please update before reporting, and
check whether the problem is still there.

| Version | Supported |
| --- | --- |
| 0.52.x (latest release) | Yes |
| Anything older | No |

## Reporting a vulnerability

**Do not open a public issue for a security problem.** Use GitHub's private
reporting instead: on the repository's **Security** tab, choose **Report a
vulnerability**. Include what you found, the steps to reproduce it, the version and
platform, and what an attacker could do with it.

You can expect an acknowledgement within a week. We will keep you informed while it
is fixed and credit you in the release notes, unless you would rather not be named.

## What counts

FormVFX runs on your own machine, so the things that matter most are the ways
something outside it could get in:

- **The engine** listens on loopback (port 8250). Anything that makes it reachable
  from another machine, or lets a web page on another origin drive it.
- **Agents and the MCP server**: an outside agent changing a project before the person
  has allowed it in the window, or while agents are refused.
- **The Terminal panel and Chat**: anything other than FormVFX's own window reaching
  a shell or Claude Code through them.
- **Plug-ins**: a WebAssembly effect plug-in getting out of its sandbox.
- **Project and template files**: opening a project, template, preset or footage file
  in a way that writes outside where it should.
- **Downloads**: an FFmpeg or model download getting past the check against its
  pinned record or SHA-256.

Bugs that need someone to already control your machine, or to run a modified build
of FormVFX, are ordinary bugs: please open an issue for those.
