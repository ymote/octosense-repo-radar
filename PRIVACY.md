# Repo Radar privacy

Repo Radar is published by [ymote](https://github.com/ymote). It searches public
GitHub repositories without a GitHub account, token, or access to private repos.
It does not star repositories, post comments, change code, or send email.

Search terms and selected repository names are sent to `api.github.com` over
HTTPS. GitHub receives the requesting device's network information under its own
privacy policy. Opening a repository or release link launches `github.com` in
the device's browser. Repo Radar does not collect analytics or send data to an
app-operated backend.

The watchlist, its order, selection, cached public responses and observation
timestamps are saved in this app's private OctoSense storage. Unpin removes a
watchlist entry; bounded cached public responses can remain until evicted.
Removing the app's data through the host removes its local state. Pins are not
GitHub stars and are not synchronized with a GitHub account.

Chat is optional. When enabled through OctoSense, questions, relevant public
repository information and requested watchlist tool results can be processed by
the model provider configured in the host. OctoSense owns the conversation and
agent consent. Repo Radar never requests or stores provider credentials. App
tools are not shared with other agents by default. Search and watchlist features
work without enabling an assistant.

The user can explicitly publish a project summary to their device's Glancer.
Repo Radar does not schedule background monitoring or send automatic notifications
in this version.

Please report issues through the [public issue tracker](https://github.com/ymote/octosense-repo-radar/issues).
Do not include tokens, passwords, personal conversations or private repository
data in reports.
