# ChartML Documentation Site

Official documentation site for ChartML - a declarative markup language for creating beautiful, interactive data visualizations.

🌐 **Live Site:** [chartml.org](https://chartml.org)

## Local Development

```bash
# Install dependencies
npm install

# Start dev server
npm run docs:dev

# Build for production
npm run docs:build

# Preview production build
npm run docs:preview
```

## Documentation Structure

```
docs/
├── index.md              # Landing page
├── spec.md               # Full ChartML specification
├── examples.md           # 42 real-world examples
├── quick-reference.md    # Syntax quick reference
├── schema.md             # JSON Schema documentation
└── public/
    ├── chartml_schema.json  # Downloadable schema
    └── logo.svg             # ChartML logo
```

## Deployment

This site is automatically deployed to Vercel on every push to main:

- **Production:** chartml.org
- **Preview:** Auto-generated URLs for PRs

## Built With

- [VitePress](https://vitepress.dev/) - Static site generator
- [Vue](https://vuejs.org/) - Framework
- [Vercel](https://vercel.com/) - Hosting & deployment

## License

MIT License - see LICENSE file for details
