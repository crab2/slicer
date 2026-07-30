# Sketch Analysis Guide: Reading Text Placeholders

**For Dog Week and All WDS Projects**

---

## Best Practice: When to Use Text vs. Markers

### Use ACTUAL TEXT for:

- **Headlines** - Provides content guidance and context
- **Button labels** - Shows intended action clearly
- **Navigation items** - Clarifies structure
- **Short, important text** - Where specific wording matters

**Example:**

```
Every walk. on time. Every time.  ← Actual text (readable)
```

**Benefits:**

- Agent can read and suggest this as starting content
- Provides context for design decisions
- Can still be changed during specification

### Use HORIZONTAL LINE MARKERS for:

- **Body paragraphs** - Content TBD, just need length indication
- **Long descriptions** - Where specific wording isn't decided yet
- **Placeholder content** - General sizing guidance

**Example:**

```
─────────────────────────────────────────  ← Line markers
─────────────────────────────────────────  ← Show length/size
─────────────────────────────────────────  ← Not final content
─────────────────────────────────────────
```

**Benefits:**

- Shows font size and capacity without committing to content
- Faster for sketching body text
- Focuses on layout, not copywriting

---

## Understanding Sketch Text Markers

In Dog Week sketches (and most UI sketches), **text is represented by horizontal lines in groups**.

### What You See

```
Page Title (centered):
       ═════════════════════════    ← Thick pair, centered = Heading, center-aligned
       ═════════════════

Body text (left-aligned):
─────────────────────────────────────────    ← Thin pairs, left edge = Body, left-aligned
─────────────────────────────────────────
─────────────────────────────────────────
─────────────────────────────────────────
─────────────────────────────────────────

Caption (right-aligned):
                            ──────────────────    ← Short pair, right edge = Caption, right-aligned
                            ──────────────────

Justified/Full-width text:
═════════════════════════════════════════════    ← Extends full width = Justified
═════════════════════════════════════════════
```

### 3. Line Count → Number of Text Lines

**Each PAIR of horizontal lines = ONE line of text**

| Number of Pairs | Text Lines | Typical Use                    |
| --------------- | ---------- | ------------------------------ |
| 1 pair          | 1 line     | Headlines, labels, buttons     |
| 2 pairs         | 2 lines    | Short headlines, subheadings   |
| 3-4 pairs       | 3-4 lines  | Intro paragraphs, descriptions |
| 5+ pairs        | 5+ lines   | Body copy, long descriptions   |

---

## Step 0: Establish Scale Using Project Context

**Before analyzing individual text elements, establish your reference points:**

### 1. Check Previous Pages in Project

If analyzing multiple pages in the same project:

**Look for established patterns:**

```
Start Page (already analyzed):
- Body text: Thin lines, icon-sized spacing → 16px Regular
- Button labels: Medium lines → 16px Semibold
- Page title: Thick lines, button-height spacing → 48px Bold

Current Page (About Page):
- Similar thin lines, icon-sized spacing → **Same: 16px Regular**
- Similar medium lines in buttons → **Same: 16px Semibold**
```

**Design System Integration:**

- If project has a design system, match visual patterns to existing components
- Body text that looks like Start Page body text → Use same specification
- Buttons that look like Start Page buttons → Use same specification

**Benefits:**

- ✅ Maintains consistency across all pages
- ✅ Builds reusable design patterns
- ✅ Reduces specification time for subsequent pages
- ✅ Creates cohesive user experience

### 2. Find UI Anchors in Current Sketch

- Browser chrome (address bar, scrollbars)
- Standard UI elements (buttons, icons, form inputs)
- Use these to calibrate scale for this specific sketch resolution

---

## Analysis Rules

### 1. Line Thickness → Font Weight (Relative)

**Line thickness indicates font weight (bold/regular), NOT font size**

**Compare lines RELATIVE to each other within the sketch:**

| Relative Thickness | Font Weight | CSS Value        | Typical Use                  |
| ------------------ | ----------- | ---------------- | ---------------------------- |
| Thickest (═══)     | Bold        | font-weight: 700 | Headlines, strong emphasis   |
| Thick (═══)        | Semibold    | font-weight: 600 | Subheadings, medium emphasis |
| Medium (──)        | Medium      | font-weight: 500 | Slightly emphasized text     |
| Thin (──)          | Regular     | font-weight: 400 | Body text, normal content    |
| Thinnest (─)       | Light       | font-weight: 300 | Subtle text, de-emphasized   |

**Don't measure pixels—compare thickness relative to other text in the same sketch.**

### 2. Distance Between Lines → Font Size (Context-Based)

**The vertical spacing between lines indicates font size—compare to UI elements**

| Spacing Relative To   | Estimated Font Size | Typical Use                         |
| --------------------- | ------------------- | ----------------------------------- |
| Button Height         | ~40-48px            | Large Heading - Page titles         |
| Address Bar Height    | ~32-40px            | Medium Heading - Section headings   |
| Between Button & Icon | ~24-32px            | Small Heading - Subsection headings |
| Icon/Scrollbar Size   | ~16-24px            | Body text / Paragraphs              |
| Half Icon Size        | ~12-16px            | Captions / Helper text              |

**⚠️ Important:** If spacing seems disproportionately large (>2x button height), verify this is text and not an image placeholder or colored box!

### 2a. Visual Examples: Text vs. Image Confusion

**TEXT - Normal spacing:**

```
═══════════════════════════════  ← Bold line
                                  ← ~Button Height
═══════════════════════════════  ← Bold line

This is clearly TEXT (H1 heading)
```

**IMAGE - Large spacing (confusion risk):**

```
═══════════════════════════════  ← Line?

                                  ← Much larger than any UI element!

═══════════════════════════════  ← Line?

This might be an IMAGE PLACEHOLDER or COLORED BOX, not text!
Ask user to confirm.
```

**When in doubt:** If spacing is disproportionately large compared to UI elements, ask: "Is this text or an image/box?"

### 3. Text Alignment → Horizontal Position

**The position of line pairs within the section indicates text alignment**

| Alignment          | Visual Indicator                         | Typical Use                       |
| ------------------ | ---------------------------------------- | --------------------------------- |
| **Left-aligned**   | Lines start at left edge of container    | Body text, lists, labels          |
| **Center-aligned** | Lines centered, equal spacing both sides | Headlines, hero text, CTAs        |
| **Right-aligned**  | Lines end at right edge of container     | Captions, metadata, prices, dates |
| **Justified**      | Lines extend full width of container     | Dense body text, formal content   |

#### Visual Examples

**Left-Aligned Text:**

```
Container: |                                                           |

═════════════════════════    ← Starts at left edge
═════════════════════════
                                              [empty space →]
```

**Center-Aligned Text:**

```
Container: |                                                           |

               ═════════════════════════    ← Centered in container
               ═════════════════════════
```

**Right-Aligned Text:**

```
Container: |                                                           |

                                          ═════════════    ← Ends at right edge
                                          ═════════════
```

**Justified/Full-Width Text:**

```
Container: |                                                           |

═════════════════════════════════════════════════════    ← Spans full width
═════════════════════════════════════════════════════
```

---

### 4. Number of Lines → Content Length

| Lines in Sketch | Content Type    | Character Estimate     |
| --------------- | --------------- | ---------------------- |
| 1-2 lines       | Heading/Title   | 20-60 characters total |
| 3-5 lines       | Short paragraph | 150-350 characters     |
| 6-10 lines      | Full paragraph  | 400-700 characters     |
| 10+ lines       | Long content    | 700+ characters        |

### 4. Line-Height Calculation

**Line-height is derived from font size and spacing:**

```
Line-height ratio = (Distance between lines) / (Estimated font size)

Example:
Distance: 28px
Font size: 24px
Line-height: 28 / 24 = 1.16 ≈ 1.2
```

**Typical ratios:**

- **1.1-1.2** = Tight (headings)
- **1.4-1.5** = Normal (body text)
- **1.6-1.8** = Loose (airy text)

```
Left-aligned:           Center-aligned:        Right-aligned:
──────────────────     ──────────────────            ──────────────────
──────────────────        ──────────────         ──────────────────
──────────────────         ──────────                ──────────────────
```

### 5. Characters Per Line

**Based on estimated font size and line width:**

```
Large Heading (~48px):  ═══════════════════     = ~20-25 chars
Medium Heading (~36px): ═══════════════════════ = ~25-30 chars
Small Heading (~24px):  ─────────────────────── = ~40-50 chars
Body Text (~16px):      ──────────────────────────────── = ~60-70 chars
Caption (~12px):        ──────────────────────────────────── = ~80-90 chars
```

---

## Dog Week Example Analysis

### Example 1: Landing Page Hero

**Sketch shows:**

```
═══════════════════════════════  ← Line 1 (thick, center)
═══════════════════════════      ← Line 2 (thick, center)
```

**Analysis:**

- **Type:** Large Heading (Page Title)
- **Lines:** 2
- **Line thickness:** Thickest in sketch → **Bold** (font-weight: 700)
- **Distance between lines:** Matches button height → **~40-48px font-size**
- **Line-height:** ~1.2 (calculated from spacing)
- **Alignment:** Center
- **Capacity:** ~25-30 chars per line = 50-60 total
- **Semantic HTML:** Determined by page structure (likely H1 if page title)

**Content Guidance:**

```
English: "Welcome to Your / Dog Care Hub" (48 chars) ✅
Swedish: "Välkommen till Din / Hundvårdshub" (50 chars) ✅
```

### Example 2: Feature Description

**Sketch shows:**

```
─────────────────────────────────────────  ← Line 1
─────────────────────────────────────────  ← Line 2
─────────────────────────────────────────  ← Line 3
─────────────────────────────────────────  ← Line 4
```

**Analysis:**

- **Type:** Body text / Paragraph
- **Lines:** 4
- **Line thickness:** Thinnest in sketch → **Regular** (font-weight: 400)
- **Distance between lines:** Matches icon/scrollbar size → **~16-20px font-size**
- **Line-height:** ~1.5 (calculated from spacing)
- **Alignment:** Left
- **Capacity:** ~60-70 chars per line = 240-280 total

**Content Guidance:**

```
English: "Organize your family around dog care. Assign walks, track
feeding schedules, and never miss a walk again. Perfect for busy
families who want to ensure their dogs get the care they need."
(206 chars) ✅

Swedish: "Organisera din familj kring hundvård. Tilldela promenader,
spåra matscheman och missa aldrig en promenad igen. Perfekt för
upptagna familjer som vill säkerställa att deras hundar får den
vård de behöver." (218 chars) ✅
```

### Example 3: Button Text

**Sketch shows:**

```
[────────────]  ← Single line inside button shape
```

**Analysis:**

- **Type:** Button label
- **Lines:** 1
- **Line thickness:** Medium (relative) → **Semibold** (font-weight: 600)
- **Estimated font-size:** ~16-18px (button standard)
- **Capacity:** ~8-12 characters

**Content Guidance:**

```
English: "Get Started" (11 chars) ✅
Swedish: "Kom Igång" (9 chars) ✅
```

---

## Agent Instructions

When analyzing sketches with text placeholders:

### Step 1: Count the Lines

```
How many horizontal bar groups do you see?
```

### Step 2: Compare Line Thickness → Font Weight

```
Line thickness indicates font weight (RELATIVE comparison):
- Thickest lines → Bold (font-weight: 700)
- Thick lines → Semibold (font-weight: 600)
- Medium lines → Medium (font-weight: 500)
- Thin lines → Regular (font-weight: 400)
- Thinnest lines → Light (font-weight: 300)
```

### Step 3: Compare Distance to UI Elements → Font Size

```
Vertical spacing relative to context anchors:
- Matches Button Height → ~40-48px font (Large Heading)
- Matches Address Bar → ~32-40px font (Medium Heading)
- Between Button & Icon → ~24-32px font (Small Heading)
- Matches Icon/Scrollbar → ~16-24px font (Body Text)
- Half Icon Size → ~12-16px font (Caption/Small Text)

⚠️ If spacing is disproportionately large, verify this is text (not image/box)!

NOTE: Visual size ≠ Semantic HTML level (H1/H2/H3).
Semantic heading levels are determined by content structure, not appearance.
```

### Step 4: Calculate Line-Height

```
Line-height = Distance between lines / Font size

Example:
- Distance: 28px
- Font size: 24px
- Line-height: 28 / 24 = 1.16 ≈ 1.2
```

### Step 5: Calculate Capacity

```
Lines × Characters-per-line = Total capacity

Example:
5 lines × 60 chars/line = 300 characters
```

### Step 6: Generate Content Guidance

```
"Content should be: [type], [line count] lines, ~[total chars] characters"

Example:
"Content should be: Full paragraph, 5 lines, ~300 characters"
```

### Step 7: Validate Content

```
If user provides content:
- Count actual characters
- Compare to capacity
- Warn if mismatch

Example:
⚠️ Your content (450 chars) exceeds sketch capacity (300 chars).
Consider shortening or increasing font size.
```

---

## Benefits of This Approach

✅ **Accurate Content Generation**

- Content matches design space
- No overflow or underflow issues
- Professional fit and finish

✅ **Design Fidelity**

- Specifications match sketches
- Developers can trust the specs
- Reduces back-and-forth

✅ **Multilingual Support**

- Can estimate if translation fits
- Adjust content length per language
- Prevents UI breaking

✅ **Quality Assurance**

- Catches content length issues early
- Ensures consistency
- Professional output

---

## Example in Action

**User shares Dog Week sketch of profile setup page**

**Agent analyzes:**

> "Looking at the profile section heading, I see 2 thick horizontal lines, center-aligned at the top.
>
> **Analysis:**
>
> - 2 lines of text (thick bars)
> - Line thickness: Thick (relative) → Bold weight
> - Spacing: Less than button height → ~28-32px font
> - Line-height: ~1.3 (calculated)
> - Alignment: Center
> - Capacity: ~50-60 characters (25-30 per line)
>
> **This heading should be:** Brief, 2 short lines, ~50-60 characters total
>
> **Suggested content:**
>
> - EN: "Complete Your / Profile Setup" (41 chars) ✅
> - SV: "Slutför Din / Profilinställning" (45 chars) ✅
>
> Both fit comfortably within the designed space. Does this match your intent?"

---

## Integration with WDS Workflow

This analysis happens automatically in:

- **4B: Sketch Analysis** - Initial detection
- **object-types/heading-text.md** - Detailed analysis
- **4C-04: Content & Languages** - Content validation
- **4C-08: Generate Spec** - Final documentation

**Result:** Every text element in WDS specifications includes accurate character capacity and content guidance! 🎨✨

---

_This guide ensures all WDS projects generate content that perfectly fits the designed space._
