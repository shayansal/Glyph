# Policy-Safe AI Personalization

Glyphspace AI adapters propose patches. They do not create authority.

The safe flow is:

1. User asks for a change.
2. A local or remote adapter proposes a patch.
3. Policy validates the patch.
4. Unsafe operations are rejected with explanations.
5. Accepted operations remain reversible and auditable.

Example: an AI request may move a close-deal confirmation closer to the current task, but it cannot hide the confirmation or make a high-risk action automatic.
