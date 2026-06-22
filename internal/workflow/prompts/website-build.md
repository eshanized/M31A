---
version: 1.0
phase: plan, execute
injected_in: plan.go/buildPlanContext, execute.go/buildExecuteContext (when scope includes "website", "web", "landing", "frontend", "nextjs", "ui")
last_reviewed: 2026-06-22
---

# Website Build Instructions (Next.js App Router)

You are building a production-quality website using Next.js App Router with React. Every decision you make — from color tokens to component structure — must serve visual excellence and complete functionality. No placeholder pages. No "coming soon" stubs. Every page gets full content, real layout, and polished visuals.

## Design System

You MUST implement a complete design system before writing any page or component. This is non-negotiable.

### Color Tokens

Define all colors in `app/globals.css` as CSS custom properties. Choose a palette based on the website type:

**SaaS / Tech / Dashboard:**
- Primary: `#6366f1` (indigo-500) — trust, professionalism
- Secondary: `#8b5cf6` (violet-500) — creativity, innovation
- Accent: `#06b6d4` (cyan-500) — energy, clarity
- Background: `#0f172a` (slate-900) dark / `#f8fafc` (slate-50) light
- Surface: `#1e293b` (slate-800) dark / `#ffffff` light
- Text primary: `#f1f5f9` (slate-100) dark / `#0f172a` (slate-900) light
- Text secondary: `#94a3b8` (slate-400) dark / `#64748b` (slate-500) light
- Border: `#334155` (slate-700) dark / `#e2e8f0` (slate-200) light
- Success: `#10b981` (emerald-500)
- Warning: `#f59e0b` (amber-500)
- Error: `#ef4444` (red-500)

**E-commerce / Marketplace:**
- Primary: `#f97316` (orange-500) — urgency, warmth
- Secondary: `#ec4899` (pink-500) — delight, playfulness
- Accent: `#14b8a6` (teal-500) — freshness, trust
- Background: `#fafaf9` (stone-50) light / `#1c1917` (stone-900) dark
- Surface: `#ffffff` light / `#292524` (stone-800) dark
- Text primary: `#1c1917` (stone-900) / `#fafaf9` (stone-50)
- Text secondary: `#78716c` (stone-500) / `#a8a29e` (stone-400)
- Border: `#e7e5e4` (stone-200) / `#44403c` (stone-700)
- Success: `#22c55e` (green-500)
- Warning: `#eab308` (yellow-500)
- Error: `#dc2626` (red-600)

**Portfolio / Creative / Agency:**
- Primary: `#a855f7` (purple-500) — creativity, imagination
- Secondary: `#f43f5e` (rose-500) — passion, boldness
- Accent: `#facc15` (yellow-400) — energy, optimism
- Background: `#09090b` (zinc-950) dark / `#fafafa` (zinc-50) light
- Surface: `#18181b` (zinc-900) dark / `#ffffff` light
- Text primary: `#fafafa` (zinc-50) / `#18181b` (zinc-900)
- Text secondary: `#a1a1aa` (zinc-400) / `#71717a` (zinc-500)
- Border: `#27272a` (zinc-800) / `#e4e4e7` (zinc-200)
- Success: `#22c55e` (green-500)
- Warning: `#f59e0b` (amber-500)
- Error: `#ef4444` (red-500)

**Blog / Content / Media:**
- Primary: `#2563eb` (blue-600) — readability, trust
- Secondary: `#7c3aed` (violet-600) — depth, knowledge
- Accent: `#059669` (emerald-600) — growth, freshness
- Background: `#ffffff` light / `#111827` (gray-900) dark
- Surface: `#f9fafb` (gray-50) light / `#1f2937` (gray-800) dark
- Text primary: `#111827` (gray-900) / `#f9fafb` (gray-50)
- Text secondary: `#6b7280` (gray-500) / `#9ca3af` (gray-400)
- Border: `#e5e7eb` (gray-200) / `#374151` (gray-700)
- Success: `#16a34a` (green-600)
- Warning: `#d97706` (amber-600)
- Error: `#dc2626` (red-600)

**Corporate / Finance / Legal:**
- Primary: `#1d4ed8` (blue-700) — stability, authority
- Secondary: `#0f766e` (teal-700) — balance, harmony
- Accent: `#b45309` (amber-700) — prestige, confidence
- Background: `#ffffff` / `#111827` (gray-900)
- Surface: `#f8fafc` (slate-50) / `#1e293b` (slate-800)
- Text primary: `#0f172a` (slate-900) / `#f1f5f9` (slate-100)
- Text secondary: `#475569` (slate-600) / `#94a3b8` (slate-400)
- Border: `#e2e8f0` (slate-200) / `#334155` (slate-700)
- Success: `#15803d` (green-700)
- Warning: `#b45309` (amber-700)
- Error: `#b91c1c` (red-700)

### Typography

Use `next/font/google` for optimal performance. Define a type scale:

```
--font-sans: Inter, system-ui, sans-serif (body)
--font-display: Cal Sans, Outfit, or Clash Display (headings) — if available, else use sans-serif with weight 700-900
--font-mono: JetBrains Mono, Fira Code (code blocks)
```

Type scale (use Tailwind defaults or custom CSS vars):
- `text-xs`: 0.75rem / 1rem — captions, labels
- `text-sm`: 0.875rem / 1.25rem — secondary text
- `text-base`: 1rem / 1.5rem — body
- `text-lg`: 1.125rem / 1.75rem — lead paragraphs
- `text-xl`: 1.25rem / 1.75rem — section subtitles
- `text-2xl`: 1.5rem / 2rem — section headings
- `text-3xl`: 1.875rem / 2.25rem — page subtitles
- `text-4xl`: 2.25rem / 2.5rem — hero subheadings
- `text-5xl`: 3rem / 1 — hero headings (leading-none for impact)
- `text-6xl`: 3.75rem / 1 — large hero (optional)

### Spacing System

Use consistent spacing. Define in CSS:
- `--space-1`: 0.25rem (4px)
- `--space-2`: 0.5rem (8px)
- `--space-3`: 0.75rem (12px)
- `--space-4`: 1rem (16px)
- `--space-6`: 1.5rem (24px)
- `--space-8`: 2rem (32px)
- `--space-12`: 3rem (48px)
- `--space-16`: 4rem (64px)
- `--space-20`: 5rem (80px)
- `--space-24`: 6rem (96px)

### Shadows and Elevation

```
--shadow-sm: 0 1px 2px rgba(0,0,0,0.05)
--shadow-md: 0 4px 6px -1px rgba(0,0,0,0.1), 0 2px 4px -2px rgba(0,0,0,0.1)
--shadow-lg: 0 10px 15px -3px rgba(0,0,0,0.1), 0 4px 6px -4px rgba(0,0,0,0.1)
--shadow-xl: 0 20px 25px -5px rgba(0,0,0,0.1), 0 8px 10px -6px rgba(0,0,0,0.1)
--shadow-glow: 0 0 40px rgba(primary, 0.15) — for hero accents
```

### Border Radius

```
--radius-sm: 0.375rem
--radius-md: 0.5rem
--radius-lg: 0.75rem
--radius-xl: 1rem
--radius-2xl: 1.5rem
--radius-full: 9999px
```

## Dark / Light Mode

Implement using CSS custom properties and `next-themes`:
1. Wrap app in `<ThemeProvider attribute="class">` from `next-themes`
2. Define all color tokens in `:root` (light) and `.dark` (dark)
3. Add a toggle button in the Navbar with sun/moon icon transition
4. Default to system preference, persist user choice in localStorage

## Component Library

Create reusable components in `components/ui/`:

### Layout Components
- **Navbar**: Sticky header with glassmorphic backdrop-blur, logo, navigation links, CTA button, mobile hamburger menu with slide-in drawer
- **Footer**: Multi-column with logo, link groups, newsletter signup form, social icons, copyright
- **Container**: Max-width wrapper (`max-w-7xl mx-auto px-4 sm:px-6 lg:px-8`)
- **Section**: Vertical padding wrapper (`py-16 sm:py-20 lg:py-24`)

### Hero Components
- **HeroSection**: Full-width with gradient background or image overlay, headline, subheadline, CTA buttons (primary + secondary), optional floating illustration or 3D element
- **HeroWithGrid**: Split layout — text left, image/grid of cards right
- **HeroWithVideo**: Background video or animated gradient with centered text

### Content Components
- **FeatureCard**: Icon + title + description, hover lift effect
- **PricingCard**: Price + features list, CTA button, "popular" badge variant
- **TestimonialCard**: Quote + author avatar + name + role
- **TeamCard**: Photo + name + role + social links
- **BlogCard**: Featured image + category tag + title + excerpt + date + read time
- **StatCard**: Large number + label + optional trend indicator
- **FAQItem**: Accordion with chevron rotation

### Interactive Components
- **Button**: Variants (primary, secondary, ghost, outline) + sizes (sm, md, lg) + loading state
- **Input**: Label + input + helper text + error state + focus ring
- **Textarea**: Multi-line input with character count
- **Select**: Dropdown with search
- **Modal**: Overlay + centered panel + close button + animation
- **Tabs**: Horizontal tabs with active indicator animation
- **Accordion**: Expand/collapse with smooth height animation

### Feedback Components
- **Badge**: Small label (e.g., "New", "Pro", category tags)
- **Alert**: Info/success/warning/error variants with icon
- **Toast**: Floating notification with auto-dismiss

### Visual Components
- **GradientText**: Text with gradient fill
- **GlowCard**: Card with subtle glow on hover
- **BentoGrid**: Asymmetric grid layout for features/showcase
- **Marquee**: Infinite scroll animation for logos or testimonials
- **AnimatedCounter**: Number counting animation on scroll into view

## Page Generation Rules

### Mandatory Pages (for a standard website)

Every website MUST include these pages with FULL content (no stubs):

1. **Home** (`app/page.tsx`) — Hero, features/ services overview, testimonials, CTA section, partners/logos strip
2. **About** (`app/about/page.tsx`) — Mission statement, team section, company timeline/values, stats
3. **Features or Services** (`app/features/page.tsx` or `app/services/page.tsx`) — Grid of feature cards, detailed descriptions, screenshots or illustrations
4. **Pricing** (`app/pricing/page.tsx`) — Pricing tiers (3 plans recommended), feature comparison table, FAQ accordion
5. **Contact** (`app/contact/page.tsx`) — Contact form (name, email, subject, message), map placeholder, office address, social links
6. **Blog** (`app/blog/page.tsx`) — Blog listing with 3-6 sample posts (realistic titles, excerpts, dates, read times), category filters
7. **Blog Post** (`app/blog/[slug]/page.tsx`) — Full article layout with author bio, related posts, share buttons

### Additional Pages (if relevant to the website type)

- **Portfolio / Work** (`app/portfolio/page.tsx`) — Project grid with hover effects, category filtering
- **Careers** (`app/careers/page.tsx`) — Job listings, company culture section
- **FAQ** (`app/faq/page.tsx`) — Accordion with questions and answers
- **Privacy Policy** (`app/privacy/page.tsx`) — Full legal text with sections
- **Terms of Service** (`app/terms/page.tsx`) — Full legal text
- **404** (`app/not-found.tsx`) — Custom 404 page with illustration and back-to-home link

### Page Content Rules

- Every page MUST have realistic placeholder content — not "Lorem ipsum"
- Use actual business names, product names, and realistic descriptions
- Every section must have a heading, subheading, and meaningful body text
- Images: Use `next/image` with solid-color placeholders or gradient backgrounds. Never leave empty `<div>` for images
- Every page must be responsive: mobile, tablet, desktop
- Every interactive element must have hover/focus states

## Layout and Navigation

### Root Layout (`app/layout.tsx`)
- `<html lang="en" suppressHydrationWarning>`
- `<body>` with font classes, `min-h-screen bg-background text-foreground`
- `<ThemeProvider>` from next-themes
- `<Navbar />` at top
- `<main className="flex-1">{children}</main>`
- `<Footer />` at bottom
- Proper `<head>` metadata: title, description, og:image, twitter:card

### Navigation Structure
- Navbar links match the pages you create
- Active link has visual indicator (underline, color change, or background)
- Mobile menu: hamburger icon → slide-in or dropdown with all links
- Smooth scroll for anchor links on the same page

## Animations and Micro-interactions

Use `framer-motion` for animations. Implement:

1. **Page transitions**: Fade-in on route change (lightweight, <200ms)
2. **Scroll animations**: Elements fade-in and slide-up as they enter viewport (`whileInView`)
3. **Hover effects**: Cards lift (translateY -4px + shadow increase), buttons scale (1.02-1.05)
4. **Stagger children**: Lists of cards/items animate in sequence with 50-100ms delay between items
5. **Loading states**: Skeleton loaders for async content, spinner for form submissions
6. **Number counters**: Animated count-up for stat sections
7. **Marquee**: Infinite scroll for logo strips or testimonial carousels
8. **Navbar**: Background opacity change on scroll (transparent → glassmorphic)
9. **Mobile menu**: Slide-in with backdrop overlay fade

### Animation Performance
- Use `transform` and `opacity` only for animations (GPU-accelerated)
- Avoid animating `width`, `height`, `margin`, `padding` — use `scale` or `clipPath` instead
- Respect `prefers-reduced-motion` media query — disable or simplify animations
- Keep animations under 300ms for interactions, under 500ms for page transitions

## Responsive Design

### Breakpoint Strategy
- Mobile first: base styles for <640px
- `sm:` (640px) — tablet portrait
- `md:` (768px) — tablet landscape
- `lg:` (1024px) — desktop
- `xl:` (1280px) — large desktop
- `2xl:` (1536px) — extra large

### Responsive Patterns
- Navbar: hamburger menu on mobile, horizontal links on desktop
- Hero: stacked layout on mobile, side-by-side on desktop
- Feature grids: 1 column mobile → 2 columns tablet → 3-4 columns desktop
- Pricing cards: stacked mobile → side-by-side desktop
- Footer: single column mobile → multi-column desktop
- Typography: smaller sizes on mobile, scale up on desktop

## SEO and Metadata

### Every Page Must Have
```tsx
export const metadata: Metadata = {
  title: "Page Title | Site Name",
  description: "Compelling meta description (150-160 chars)",
  openGraph: {
    title: "...",
    description: "...",
    images: [{ url: "/og-image.png", width: 1200, height: 630 }],
  },
};
```

### Technical SEO
- Semantic HTML: `<header>`, `<nav>`, `<main>`, `<section>`, `<article>`, `<footer>`
- Proper heading hierarchy: one `<h1>` per page, sequential `<h2>` → `<h3>` etc.
- `alt` text on all images
- `aria-label` on interactive elements
- `<link rel="canonical">` on each page
- `robots.txt` and `sitemap.xml` (if static generation)

## File Structure

```
app/
  layout.tsx          — root layout with providers, fonts, navbar, footer
  page.tsx            — home page
  globals.css         — design tokens, base styles, animations
  about/
    page.tsx
  features/
    page.tsx
  pricing/
    page.tsx
  contact/
    page.tsx
  blog/
    page.tsx
    [slug]/
      page.tsx
  not-found.tsx       — custom 404
components/
  ui/
    button.tsx
    card.tsx
    input.tsx
    textarea.tsx
    badge.tsx
    modal.tsx
    accordion.tsx
    tabs.tsx
  layout/
    navbar.tsx
    footer.tsx
    container.tsx
    mobile-menu.tsx
  sections/
    hero.tsx
    features.tsx
    pricing.tsx
    testimonials.tsx
    cta.tsx
    stats.tsx
    partners.tsx
    faq.tsx
    team.tsx
    newsletter.tsx
lib/
  utils.ts            — cn() helper, formatters
  constants.ts        — site config, navigation links, pricing data
hooks/
  use-media-query.ts  — responsive breakpoint hook
  use-scroll.ts       — scroll position hook
public/
  (placeholder images or gradient SVGs)
```

## Implementation Order

1. **Copy template files** from the extracted template directory to the working directory. The template includes: package.json, next.config.ts, tsconfig.json, postcss.config.mjs, app/globals.css, app/layout.tsx, app/page.tsx, app/not-found.tsx, lib/utils.ts, lib/constants.ts, hooks/use-media-query.ts, hooks/use-scroll.ts
2. **Install dependencies**: `npm install`
3. **Customize globals.css** color tokens to match the website type (see Color Tokens section above)
4. **Customize lib/constants.ts** with the actual site name, tagline, navigation links, pricing, features, testimonials
5. **Create UI components** in `components/ui/` (button, card, input, textarea, badge, modal, accordion, tabs)
6. **Create layout components** (navbar, footer, mobile-menu)
7. **Create section components** (hero, features, pricing, testimonials, cta, stats, partners, faq, team, newsletter)
8. **Create pages** (home → about → features → pricing → contact → blog → 404) — each with FULL content
9. **Add animations** with framer-motion (scroll reveals, hover effects, stagger, marquee)
10. **Test responsive** at all breakpoints
11. **Verify dark/light mode** toggle works
12. **Run `npm run build`** to verify no errors

## Critical Rules

- **Use the template.** Copy all files from the extracted template directory before writing any code. The template provides the foundation — customize it, don't recreate it.
- **No placeholder pages.** Every page gets real content, real layout, real styling.
- **No empty divs.** Every visual element must have content or a styled background.
- **No broken imports.** Read `package.json` before using any library. Only use dependencies you installed.
- **No accessibility violations.** Use semantic HTML, ARIA labels, focus management, and keyboard navigation.
- **No animation jank.** Use GPU-accelerated properties only. Respect reduced-motion.
- **No layout shift.** Set explicit dimensions on images. Use skeleton loaders for async content.
- **No monochrome.** Every page must use the color palette. Backgrounds, borders, text, accents — all should use tokens.
- **No orphaned components.** Every component you create must be used in at least one page.
