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

### Added
- **Enhanced scroll controls**:
  - `UP/DOWN` arrows: Scroll one message at a time
  - `PAGE UP/PAGE DOWN`: Scroll 10 messages at a time
  - `END` key: Jump to the latest message (bottom)
  - Visual scroll indicator showing current position (e.g., "[5/20] ↑↓ to scroll | END to bottom")
  
- **Improved UX**:
  - Selected message highlighted with yellow color and `>>` indicator
  - Scroll position indicator in the messages panel title
  - Auto-scroll intelligently follows new messages when at bottom

### Changed
- `App.message_scroll` type changed from `u16` to `usize` for better indexing
- Added `App.auto_scroll` field to track auto-scroll state
- Added helper methods: `scroll_up()`, `scroll_down()`, `scroll_to_bottom()`
