# Controls

OrangeDeck accepts touch, mouse, controller and keyboard input. Controller input is processed only while the app is focused; queued input from before refocusing is discarded.

| Input | Action |
| --- | --- |
| D-pad / left stick | Move focus through the five-column, two-row grid or a list |
| A | Open the focused key; inside a response dialog, approve the displayed request once |
| B | Back; inside a response dialog with an approval, reject the displayed request once |
| X | Refresh; close an open dialog without deciding |
| Y | Open Agents |
| LB / RB | Previous / next tab, while no dialog is open |
| LT / RT | Switch project/conversation; inside a response dialog, cycle pending requests |

The four tabs are LIVE, Agents, Projects and Conversations. Agents contains responses, approvals and changed files. Each deck column is pinned to a stable conversation independently of the other tabs' selected project.

## Five pairs of keys

Tap an upper **+** key to choose a conversation and label. **Assign chats** or right-click an upper key changes its assignment. **Clear assignment** removes it. The picker saves without executing a command or deciding an obscured request. D-pad chooses, A assigns and B cancels.

Both keys in a column pulse for a new current-turn response or approval. **Sound on/off** enables or mutes the notification sound. Notices do not automatically switch tabs or open dialogs.

- **Upper key:** shows the question, response and pending decisions for that same conversation and turn.
- **Lower key:** opens only a list of changed file names, full paths and recorded line numbers. Select a file to open it in the Mac editor at its recorded changed line, or the first line when no location is recorded. Code contents are viewed in the Mac editor. Deleted files remain listed without an open action.
- **No file changes:** disables the lower key only after a complete, empty edit record is confirmed.
- **Check file records / Loading:** opens a recovery dialog and reads the same conversation again. Fresh records update the list for that same turn without launching the editor; errors remain visible with retry. Cached lists remain viewable after disconnection.
- **Outside tap / Close / X / Escape:** dismisses without deciding. Input does not pass through to a background key.

A response dialog closes after approval/rejection delivery is acknowledged. During sending, both decisions are locked. Failures keep the dialog open for review and explicit retry. A new turn or replacement request cannot receive input intended for an older request, including a pointer press started before replacement.

The file dialog uses the selected Codex conversation’s working folder automatically. Opening a recorded edit requires no folder registration. Errors and **Try again** remain above the file list. If an older Mac Connector requires registration, the dialog explains how to update it. The file dialog contains no Codex approval controls. Without a received approval, the response dialog says that no approval request has been received. A opens the focused file, up/down only moves focus, and B closes. Editor requests are serialized; only an explicitly selected file can be queued after the previous handoff. Opening, refreshing or reopening the list does not launch the editor. Closing discards unsent navigation.

Long command, permission and file-change approval details remain complete inside the scrollable details area. Keyboard Enter/Escape/Space never decide, including on a focused approval button. Use a deliberate touch/click or fresh controller A/B press. Ordinary Codex questions are answered on the Mac.

## Language and preferences

Use **한국어 / EN** at the top right. User project names and conversation contents are not translated. Language, five conversation assignments and sound are saved privately in `ui-preferences.toml` beside the active config. Demo settings last only for the session. The former individual action-key assignments are no longer used by the paired deck.

Keyboard fallbacks: arrows for navigation, Q/E for tabs, Z/C for project/conversation selection. Input is isolated while a picker/dialog or text editor is open. Steam Input/InputPlumber may expose a virtual controller; touch and mouse work independently of controller detection.
