# Changelog

## [Unreleased]

### Fixed
- **Chat scrolling**: Fixed chat message display when messages exceed the visible area
  - Messages now properly scroll with intelligent height calculation
  - Removed fixed-height message boxes that caused overflow
  - Added auto-scroll feature that follows new messages automatically
  - Auto-scroll disables when manually scrolling up, re-enables when scrolling to bottom

- **Long message wrapping**: Fixed long messages being cut off
  - Messages now wrap properly to multiple lines based on terminal width
  - Each message takes only the space it needs (dynamic height)
  - Scroll calculation accounts for multi-line messages
  - No more truncated text - all content is visible

- **Input cursor positioning**: Fixed cursor placement when input text wraps
  - Cursor now correctly positioned on wrapped lines
  - Uses character count instead of byte count for accurate positioning
  - Auto-scrolls input area when typing beyond visible lines
  - Handles multi-line input properly with bounds checking
  - Supports full cursor movement (left, right, up, down)
  - Can edit text at any position, not just at the end

### Added
- **Focus system**: Switch between Message List and Input Field
  - Press `SHIFT+TAB` to toggle focus between sections
  - Visual indicators show which section is focused (`[FOCUSED]`)
  - Focused section has colored border (Cyan for messages, Green for input)
  - Cursor only visible when input field is focused

- **Enhanced input controls** (when input is focused):
  - `LEFT/RIGHT` arrows: Move cursor horizontally
  - `UP/DOWN` arrows: Move cursor vertically in multi-line input
  - `HOME`: Jump to start of input
  - `END`: Jump to end of input
  - `DELETE`: Delete character after cursor
  - Full cursor positioning support for editing anywhere in the text

- **Enhanced message list controls** (when message list is focused):
  - `UP/DOWN` arrows: Scroll one message at a time
  - `PAGE UP/PAGE DOWN`: Scroll 10 messages at a time
  - `HOME`: Jump to first message
  - `END`: Jump to latest message
  - Visual scroll indicator showing current position

### Changed
- `App.message_scroll` type changed from `u16` to `usize` for better indexing
- Added `App.auto_scroll` field to track auto-scroll state
- Added helper methods: `scroll_up()`, `scroll_down()`, `scroll_to_bottom()`
