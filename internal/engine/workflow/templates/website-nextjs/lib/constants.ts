export const SITE = {
  name: "Acme Inc",
  tagline: "Build something amazing",
  description:
    "A modern website built with Next.js, Tailwind CSS, and Framer Motion.",
  url: "https://example.com",
  email: "hello@example.com",
  phone: "+1 (555) 123-4567",
  address: "123 Main Street, San Francisco, CA 94105",
} as const;

export const NAV_LINKS = [
  { label: "Home", href: "/" },
  { label: "About", href: "/about" },
  { label: "Features", href: "/features" },
  { label: "Pricing", href: "/pricing" },
  { label: "Blog", href: "/blog" },
  { label: "Contact", href: "/contact" },
] as const;

export const SOCIAL_LINKS = [
  { label: "Twitter", href: "https://twitter.com", icon: "twitter" },
  { label: "GitHub", href: "https://github.com", icon: "github" },
  { label: "LinkedIn", href: "https://linkedin.com", icon: "linkedin" },
] as const;

export const PLANS = [
  {
    name: "Starter",
    price: "$9",
    period: "/month",
    description: "Perfect for individuals and small projects.",
    features: [
      "5 projects",
      "10 GB storage",
      "Basic analytics",
      "Email support",
      "API access",
    ],
    cta: "Get Started",
    popular: false,
  },
  {
    name: "Pro",
    price: "$29",
    period: "/month",
    description: "Best for growing teams and businesses.",
    features: [
      "Unlimited projects",
      "100 GB storage",
      "Advanced analytics",
      "Priority support",
      "API access",
      "Custom domain",
      "Team collaboration",
    ],
    cta: "Start Free Trial",
    popular: true,
  },
  {
    name: "Enterprise",
    price: "$99",
    period: "/month",
    description: "For large organizations with custom needs.",
    features: [
      "Everything in Pro",
      "Unlimited storage",
      "Custom integrations",
      "Dedicated support",
      "SLA guarantee",
      "SSO & SAML",
      "Audit logs",
    ],
    cta: "Contact Sales",
    popular: false,
  },
] as const;

export const FEATURES = [
  {
    title: "Lightning Fast",
    description:
      "Built on Next.js for blazing fast page loads and optimal performance.",
    icon: "zap",
  },
  {
    title: "Secure by Default",
    description:
      "Enterprise-grade security with SOC 2 compliance and end-to-end encryption.",
    icon: "shield",
  },
  {
    title: "Easy Integration",
    description:
      "Connect with your favorite tools through our robust API and webhooks.",
    icon: "puzzle",
  },
  {
    title: "Real-time Analytics",
    description:
      "Monitor your performance with live dashboards and customizable reports.",
    icon: "bar-chart",
  },
  {
    title: "Team Collaboration",
    description:
      "Work together seamlessly with role-based access and real-time editing.",
    icon: "users",
  },
  {
    title: "24/7 Support",
    description:
      "Get help whenever you need it with our dedicated support team.",
    icon: "headphones",
  },
] as const;

export const TESTIMONIALS = [
  {
    quote:
      "This product has transformed how we work. The team collaboration features are incredible.",
    author: "Sarah Chen",
    role: "CTO, TechCorp",
    avatar: "/avatars/sarah.jpg",
  },
  {
    quote:
      "We switched from our old tool and saw a 40% improvement in productivity immediately.",
    author: "Marcus Johnson",
    role: "Engineering Lead, StartupXYZ",
    avatar: "/avatars/marcus.jpg",
  },
  {
    quote:
      "The analytics dashboard alone is worth the price. We finally have visibility into our metrics.",
    author: "Emily Rodriguez",
    role: "Product Manager, DataCo",
    avatar: "/avatars/emily.jpg",
  },
] as const;

export const STATS = [
  { value: "10K+", label: "Active Users" },
  { value: "99.9%", label: "Uptime" },
  { value: "50M+", label: "API Requests/Day" },
  { value: "4.9/5", label: "Customer Rating" },
] as const;
