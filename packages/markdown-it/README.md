# @chartml/markdown-it

A markdown-it plugin for automatically rendering ChartML visualizations from code blocks.

## Installation

```bash
npm install @chartml/markdown-it @chartml/core
```

## Usage

### With VitePress

```javascript
// .vitepress/config.js
import chartMLPlugin from '@chartml/markdown-it';

export default {
  markdown: {
    config: (md) => {
      md.use(chartMLPlugin);
    }
  }
};
```

### With markdown-it directly

```javascript
import markdownIt from 'markdown-it';
import chartMLPlugin from '@chartml/markdown-it';

const md = markdownIt();
md.use(chartMLPlugin);

const html = md.render('```chartml\ndata:\n  - x: 1\n    y: 2\n```');
```

## Markdown Syntax

Simply use `chartml` as the language identifier in your code blocks:

````markdown
```chartml
data:
  - month: Jan
    revenue: 45000
  - month: Feb
    revenue: 52000

visualize:
  type: bar
  columns: month
  rows: revenue
  style:
    title: Monthly Revenue
```
````

The chart will automatically render when the page loads.

## How It Works

1. The plugin detects ` ```chartml ` code blocks during markdown parsing
2. It replaces them with a `<div>` container with the ChartML spec in a `data-chartml-spec` attribute
3. A client-side script (loaded separately) finds these containers and renders the charts using `@chartml/core`

## License

MIT
