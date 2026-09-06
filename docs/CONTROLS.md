# Controls

OrangeDeck accepts touch, controller, and keyboard input. Controller input is processed
only while the OrangeDeck window is focused.

| Physical input | Action |
| --- | --- |
| D-pad / left stick | Move focus |
| A | Shortcuts: execute focused key; Notifications: approve the displayed request once |
| B | Back; reject when an approval banner is active |
| X | Refresh/context action |
| Y | Open current query in Notifications |
| LB / RB | Previous / next page |
| LT / RT | LIVE/Shortcuts/Projects: switch project; Conversations: switch thread; displayed approval: switch request |

Keyboard fallbacks are arrow keys, Enter, Escape, X, Y, Q/E for pages, and Z/C for
threads. Every primary action is also available through a touch target.
Keyboard page/thread shortcuts are suspended while editing a prompt. Background
controller events and events queued while refocusing are discarded.
The default LIVE screen monitors one project. A / Y or touching the current question opens
its single-query Notifications tab. LIVE ‹ / › and LT / RT switch projects; the top project
picker and Projects tab share the same selection. Recent automatic mode follows activity
only within that project. The Conversations tab lists that project's threads newest first;
LT / RT selects a thread there. Y opens the current-query view, without a history dialog.

Approval banners have priority over normal A/B behavior. The banner names the action,
shows available command/path/reason details, and states that approval applies to one
action only. There is no hold-to-auto-approve behavior.
Keyboard Enter/Escape and keyboard activation of a focused banner button do not
submit approval decisions. Use a deliberate touch/click or a fresh controller A/B press.

Steam Input or InputPlumber may expose a virtual controller instead of the physical
device. On the inspected Ally, gilrs detected the InputPlumber Xbox-compatible device;
keyboard and touch remain available if controller access changes.

## Five tabs (0.1.14)

LB/RB (Q/E) cycles LIVE, 단축키, 프로젝트들, 대화, 알림. The fifth tab shows one current question
and its corresponding reply, with a pulsing border for a decision request, approval or
completed response. 확인했어요 acknowledges only the current query's notifications.

A/B decides only a request displayed on Shortcuts or Notifications. Shortcuts uses ten
square keys in five columns and two rows: key 01 approves, key 02 rejects, keys 03–10
are unassigned. D-pad/stick navigation follows the grid. A executes the focused key;
B rejects the displayed request. The Notifications page keeps its direct A approve / B reject.
LT/RT cycles requests within that query. Sending locks both decisions
until acknowledgement/resolution; failures allow explicit retry. Keyboard Enter/Escape
never decide approvals. A newly arrived or newly selected request must have been rendered
before a controller decision can apply. Reconnect requires a fresh snapshot. Ordinary
input questions are answered in Mac Codex.

A new approval for the selected conversation's current turn opens Shortcuts once and pulses
both decision keys. Its summary is visible above the grid; Request details opens Notifications.
Other projects/turns and ordinary completion notices do not trigger this page switch. Approval
announcements do not open a modal over the keys. A pointer press begun on an older request
cannot decide a replacement request when released. Keyboard Enter/Escape/Space never decide.

In 0.1.15 both decision keys stay neutral gray without a request, while disconnected,
before a new request is armed, and during sending. Each key names its state. Only actionable
requests light the green/pink keys and enable their identical hover/press feedback and hand
cursor. Controller focus uses a small A badge during actionable requests; it no longer
lights the first idle key or overrides pointer feedback.
