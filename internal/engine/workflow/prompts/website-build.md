---
version: 2.0
phase: plan, execute
injected_in: plan.go/buildPlanContext, execute.go/buildExecuteContext (when scope includes "website", "web", "landing", "frontend", "nextjs", "ui")
last_reviewed: 2026-06-25
---

# Website Build Instructions (Next.js App Router)

Build production-quality websites with Next.js App Router + React. No placeholder pages, no "coming soon" stubs. Every page gets full content, real layout, and polished visuals.

## Design System

Define all colors in `app/globals.css` as CSS custom properties. Choose ONE palette based on website type:

### Color Tokens

| Type | Primary | Secondary | Accent | Dark BG | Light BG |
|------|---------|-----------|--------|---------|----------|
| SaaS/Tech | `#6366f1` indigo-500 | `#8b5cf6` violet-500 | `#06b6d4` cyan-500 | `#0f172a` slate-900 | `#f8fafc` slate-50 |
| E-commerce | `#f97316` orange-500 | `#ec4899` pink-500 | `#14b8a6` teal-500 | `#1c1917` stone-900 | `#fafaf9` stone-50 |
| Portfolio/Creative | `#a855f7` purple-500 | `#f43f5e` rose-500 | `#facc15` yellow-400 | `#09090b` zinc-950 | `#fafafa` zinc-50 |
| Blog/Content | `#2563eb` blue-600 | `#7c3aed` violet-600 | `#059669` emerald-600 | `#111827` gray-900 | `#ffffff` |
| Corporate/Finance | `#1d4ed8` blue-700 | `#0f766e` teal-700 | `#b45309` amber-700 | `#111827` gray-900 | `#ffffff` |

Each palette also needs: surface (card bg), text-primary, text-secondary, border, success, warning, error tokens. Define both `:root` (light) and `.dark` (dark) variants.

### Typography

Use `next/font/google`: Inter (body), a display font for headings (Outfit/Clash Display if available), JetBrains Mono for code. Use Tailwind's type scale (`text-xs` through `text-6xl`).

### Dark/Light Mode

Already configured via `ThemeProvider` from `next-themes` in `app/layout.tsx`. Add a toggle button in the Navbar with sun/moon icon. Color tokens in `:root` (light) and `.dark` (dark) handle switching.

## Pre-built Components (shadcn/ui)

34 components in `components/ui/`. **Import from `@/components/ui/<component>` — do NOT recreate.** Example: `import { Button } from "@/components/ui/button"`

| Component | File | Key Exports |
|-----------|------|-------------|
| Accordion | `accordion.tsx` | `Accordion`, `AccordionItem`, `AccordionTrigger`, `AccordionContent` |
| Alert | `alert.tsx` | `Alert`, `AlertTitle`, `AlertDescription` — variants: `default`, `destructive` |
| AlertDialog | `alert-dialog.tsx` | `AlertDialog`, `AlertDialogTrigger`, `AlertDialogContent`, `AlertDialogAction`, `AlertDialogCancel` |
| AspectRatio | `aspect-ratio.tsx` | `AspectRatio` |
| Avatar | `avatar.tsx` | `Avatar`, `AvatarImage`, `AvatarFallback` |
| Badge | `badge.tsx` | `Badge` — variants: `default`, `secondary`, `destructive`, `outline` |
| Breadcrumb | `breadcrumb.tsx` | `Breadcrumb`, `BreadcrumbList`, `BreadcrumbItem`, `BreadcrumbLink`, `BreadcrumbPage`, `BreadcrumbSeparator` |
| Button | `button.tsx` | `Button` — variants: `default`, `destructive`, `outline`, `secondary`, `ghost`, `link`; sizes: `sm`, `default`, `lg`, `icon` |
| Card | `card.tsx` | `Card`, `CardHeader`, `CardTitle`, `CardDescription`, `CardContent`, `CardFooter` |
| Checkbox | `checkbox.tsx` | `Checkbox` |
| Dialog | `dialog.tsx` | `Dialog`, `DialogTrigger`, `DialogContent`, `DialogHeader`, `DialogFooter`, `DialogTitle`, `DialogDescription` |
| DropdownMenu | `dropdown-menu.tsx` | `DropdownMenu`, `DropdownMenuTrigger`, `DropdownMenuContent`, `DropdownMenuItem`, `DropdownMenuCheckboxItem`, `DropdownMenuSeparator` |
| HoverCard | `hover-card.tsx` | `HoverCard`, `HoverCardTrigger`, `HoverCardContent` |
| Input | `input.tsx` | `Input` |
| Label | `label.tsx` | `Label` |
| NavigationMenu | `navigation-menu.tsx` | `NavigationMenu`, `NavigationMenuList`, `NavigationMenuItem`, `NavigationMenuTrigger`, `NavigationMenuContent`, `NavigationMenuLink` |
| Pagination | `pagination.tsx` | `Pagination`, `PaginationContent`, `PaginationItem`, `PaginationLink`, `PaginationNext`, `PaginationPrevious` |
| Popover | `popover.tsx` | `Popover`, `PopoverTrigger`, `PopoverContent` |
| Progress | `progress.tsx` | `Progress` |
| RadioGroup | `radio-group.tsx` | `RadioGroup`, `RadioGroupItem` |
| ScrollArea | `scroll-area.tsx` | `ScrollArea`, `ScrollBar` |
| Select | `select.tsx` | `Select`, `SelectTrigger`, `SelectContent`, `SelectItem`, `SelectValue` |
| Separator | `separator.tsx` | `Separator` |
| Sheet | `sheet.tsx` | `Sheet`, `SheetTrigger`, `SheetContent`, `SheetHeader`, `SheetFooter` — sides: `top`, `bottom`, `left`, `right` |
| Skeleton | `skeleton.tsx` | `Skeleton` |
| Slider | `slider.tsx` | `Slider` |
| Switch | `switch.tsx` | `Switch` |
| Table | `table.tsx` | `Table`, `TableHeader`, `TableBody`, `TableRow`, `TableHead`, `TableCell`, `TableCaption` |
| Tabs | `tabs.tsx` | `Tabs`, `TabsList`, `TabsTrigger`, `TabsContent` |
| Textarea | `textarea.tsx` | `Textarea` |
| Toggle | `toggle.tsx` | `Toggle` — variants: `default`, `outline`; sizes: `sm`, `default`, `lg` |
| ToggleGroup | `toggle-group.tsx` | `ToggleGroup`, `ToggleGroupItem` |
| Toaster | `sonner.tsx` | `Toaster` (sonner) — already in layout |
| Tooltip | `tooltip.tsx` | `Tooltip`, `TooltipTrigger`, `TooltipContent`, `TooltipProvider` |

## Custom Components to Create

Build these in `components/sections/` and `components/layout/`:

**Layout**: Navbar (sticky, glassmorphic backdrop-blur, `NavigationMenu` + `Sheet` for mobile), Footer (multi-column, newsletter form, social icons), Container (`max-w-7xl mx-auto px-4 sm:px-6 lg:px-8`), Section (`py-16 sm:py-20 lg:py-24`)

**Hero**: HeroSection (gradient bg, headline, CTAs with `Button`), HeroWithGrid (split layout), HeroWithVideo (animated gradient)

**Content**: FeatureCard (icon + title + desc, hover lift), PricingCard (price + features + `Badge`), TestimonialCard (quote + avatar), TeamCard, BlogCard, StatCard, FAQSection (`Accordion`)

**Interactive**: ContactForm (`Input` + `Textarea` + `Label` + `Select` + `Button`), NewsletterForm (`Input` + `Button`)

**Visual**: GradientText, GlowCard, BentoGrid, Marquee, AnimatedCounter

## Page Generation Rules

Every website MUST include these pages with FULL content (no stubs):

1. **Home** (`app/page.tsx`) — Hero, features/services, testimonials, CTA, partners strip
2. **About** (`app/about/page.tsx`) — Mission, team, values, stats
3. **Features/Services** (`app/features/page.tsx`) — Feature grid with descriptions
4. **Pricing** (`app/pricing/page.tsx`) — 3 tiers, feature comparison, FAQ
5. **Contact** (`app/contact/page.tsx`) — Form (name, email, subject, message), address, socials
6. **Blog** (`app/blog/page.tsx`) — 3-6 sample posts with titles, excerpts, dates
7. **Blog Post** (`app/blog/[slug]/page.tsx`) — Full article, author bio, related posts
8. **404** (`app/not-found.tsx`) — Custom 404 with back-to-home

**Content rules**: Realistic content (no Lorem ipsum), actual business names, every section has heading + subheading + body, `next/image` with colored placeholders, responsive at all breakpoints, hover/focus states on all interactive elements.

## Animations (framer-motion)

- Page transitions: fade-in <200ms on route change
- Scroll reveals: `whileInView` fade-in + slide-up
- Hover: cards lift (translateY -4px + shadow), buttons scale (1.02-1.05)
- Stagger: 50-100ms delay between list items
- Loading: `Skeleton` for async content, spinner for form submissions
- Stat counters: animated count-up on scroll
- Navbar: opacity change on scroll (transparent → glassmorphic)
- Mobile menu: slide-in with backdrop fade
- Use `transform`/`opacity` only (GPU-accelerated), respect `prefers-reduced-motion`

## SEO

Every page must export `metadata` with `title`, `description` (150-160 chars), and `openGraph`. Use semantic HTML (`header`, `nav`, `main`, `section`, `article`, `footer`), one `<h1>` per page, `alt` on images, `aria-label` on interactive elements.

## Implementation Order

1. Copy template files from extracted directory to working directory
2. `npm install`
3. Customize `globals.css` color tokens for the website type
4. Customize `lib/constants.ts` (site name, nav links, pricing, features, testimonials)
5. Create layout components (navbar, footer, container, mobile-menu)
6. Create section components (hero, features, pricing, testimonials, cta, stats, faq, team, newsletter)
7. Create pages (home → about → features → pricing → contact → blog → 404)
8. Add framer-motion animations
9. Test responsive at all breakpoints
10. Verify dark/light mode toggle
11. `npm run build` — no errors

## Critical Rules

- **Use the template.** Copy all files before writing code. Customize, don't recreate.
- **No placeholder pages.** Real content, real layout, real styling.
- **No empty divs.** Content or styled background on every visual element.
- **No broken imports.** Check `package.json` before using any library.
- **No accessibility violations.** Semantic HTML, ARIA labels, focus management, keyboard nav.
- **No animation jank.** GPU-accelerated properties only. Respect reduced-motion.
- **No layout shift.** Explicit image dimensions, skeleton loaders for async content.
- **No monochrome.** Use color palette tokens everywhere — backgrounds, borders, text, accents.
- **No orphaned components.** Every component you create must be used in at least one page.
