# HTML Tags vs. Visual Text Styles

**Critical Best Practice for WDS Specifications**

---

## The Two-Layer System

### Layer 1: HTML Semantic Structure (h1-h6, p, etc.)

**Purpose:** SEO, accessibility, document outline, screen readers

**Rules:**

- **Each page must have exactly ONE h1** (main page title)
- **Heading hierarchy must be logical** (h1 → h2 → h3, no skipping)
- **Same across all pages** for semantic consistency
- **Not about visual appearance**

### Layer 2: Visual Text Styles (Design System)

**Purpose:** Visual hierarchy, branding, design consistency

**Rules:**

- **Named by visual purpose** (Display-Large, Headline-Primary, Body-Regular, etc.)
- **Can be applied to any HTML tag**
- **Different pages can use different visual styles** for the same HTML tag
- **About appearance, not semantics**

---

## Why Separate?

### Problem: Mixing HTML and Visual Styles

```markdown
❌ BAD:

- **Style**: H1 heading

What does this mean?

- Is it an h1 tag?
- Is it a visual style that looks like an h1?
- What if another page needs h1 but different visual style?
```

### Solution: Specify Both Independently

```markdown
✅ GOOD:

- **HTML Tag**: h1 (semantic structure)
- **Visual Style**: Display-Large (from Design System)
```

**Now we know:**

- HTML: This is the main page heading (h1 for SEO)
- Visual: It uses the "Display-Large" design system style
- Another page could have: h1 + Headline-Medium (different visual, same semantic)

---

## Real-World Examples

### Example 1: Landing Page vs. Article Page

**Landing Page - Hero Headline:**

```markdown
- **HTML Tag**: h1
- **Visual Style**: Hero headline
- **Font**: Bold, 56px, line-height 1.1
```

**Article Page - Article Title:**

```markdown
- **HTML Tag**: h1
- **Visual Style**: Main header
- **Font**: Bold, 32px, line-height 1.3
```

**Both are h1 (semantic), but different visual styles!**

### Example 2: Same Visual Style, Different Semantics

**Section Heading:**

```markdown
- **HTML Tag**: h2
- **Visual Style**: Sub header
- **Font**: Bold, 28px, line-height 1.2
```

**Testimonial Quote:**

```markdown
- **HTML Tag**: p
- **Visual Style**: Sub header
- **Font**: Bold, 28px, line-height 1.2
```

**Same visual style (Sub header), but different HTML tags for proper semantics!**

---

## Design System Visual Style Naming

### Good Visual Style Names (Descriptive & Purpose-Based)

**For Headers:**
✅ **Main header** - Primary page header
✅ **Sub header** - Section headers
✅ **Sub header light** - Lighter variant of section header
✅ **Card header** - Headers within cards
✅ **Small header** - Minor headers, labels

**For Body Text:**
✅ **Body text** - Standard paragraph text
✅ **Body text large** - Larger body text for emphasis
✅ **Body text small** - Smaller body text, secondary info
✅ **Intro text** - Opening paragraph, lead text

**For Special Purposes:**
✅ **Hero headline** - Large display text for hero sections
✅ **Caption text** - Image captions, metadata
✅ **Label text** - Form labels, UI labels
✅ **Error text** - Error messages
✅ **Success text** - Success messages
✅ **Link text** - Link styling
✅ **Button text** - Text within buttons

### Bad Visual Style Names

❌ **H1-Style** / **Heading-1** - Confuses with HTML tags
❌ **Text-Size-42** - Just a number, not semantic
❌ **Big-Text** - Too vague
❌ **Display-Large** - Too abstract (unless using design system tokens)

---

## WDS Specification Format

### Complete Example

```markdown
#### Primary Headline

**OBJECT ID**: `start-hero-headline`

**HTML Structure:**

- **Tag**: h1
- **Purpose**: Main page heading (SEO/accessibility)

**Visual Style:**

- **Style Name**: Hero headline
- **Font weight**: Bold (from 3px thick line markers in sketch)
- **Font size**: 56px (est. from 32px spacing between line pairs)
- **Line-height**: 1.1 (est. calculated from font size)
- **Color**: #1a1a1a
- **Letter spacing**: -0.02em

**Position**: Center of hero section, above supporting text

**Behavior**: Updates with language toggle

**Content**:

- EN: "Every walk. on time. Every time."
- SE: "Varje promenad. i tid. Varje gång."
```

---

## Benefits of This Approach

✅ **Flexibility** - Different pages can have different visual styles for same semantic tags
✅ **Consistency** - Design system ensures visual consistency across visual styles
✅ **SEO/Accessibility** - Proper HTML structure maintained
✅ **Scalability** - Easy to add new visual styles without breaking semantic structure
✅ **Clarity** - Designers and developers both understand the specification
✅ **Reusability** - Visual styles can be reused across different HTML tags

---

## Common Patterns

### Pattern 1: Landing Page

```
h1 → Hero headline (big hero text, 56px)
h2 → Sub header (section headings, 32px)
h3 → Small header (subsection headings, 24px)
p → Body text (regular paragraphs, 16px)
```

### Pattern 2: Blog Post

```
h1 → Main header (article title, 36px)
h2 → Sub header (section headings, 28px)
h3 → Sub header light (subsection headings, 22px)
p → Body text large (article body, 18px)
```

### Pattern 3: Dashboard

```
h1 → Main header (page title, 28px)
h2 → Card header (widget titles, 20px)
h3 → Small header (section labels, 16px)
p → Body text small (compact info, 14px)
```

**Same HTML structure (h1, h2, h3, p) but different visual styles for each context!**

---

## Implementation Note

When generating HTML prototypes or handing off to developers:

```html
<!-- The HTML tag is semantic, the class references the visual style -->
<h1 class="hero-headline">Every walk. on time. Every time.</h1>

<!-- Another page might have -->
<h1 class="main-header">Welcome to Your Profile</h1>

<!-- NOT this (mixing concerns) -->
<h1 class="h1">Every walk. on time. Every time.</h1>
```

The CSS class references the **visual style name** (hero-headline, main-header), not the HTML tag.

---

**Remember:** HTML tags = Document structure. Visual styles = Appearance. Keep them separate! 🎯
