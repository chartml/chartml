# ChartML JSON Schema

The ChartML JSON Schema provides type-safe validation for your chart specifications. It's used by editors, validators, and rendering engines to ensure ChartML documents are correct.

## Download

- **[Download chartml_schema.json](/chartml_schema.json)** (31KB)
- **Version:** 1.0.0
- **Format:** JSON Schema Draft 7

## Using the Schema

### In Your Editor

Most modern editors support JSON Schema for YAML files:

**VS Code** (with YAML extension):
```json
// .vscode/settings.json
{
  "yaml.schemas": {
    "https://chartml.org/chartml_schema.json": "*.chartml.yaml"
  }
}
```

**Monaco Editor**:
```javascript
import schema from './chartml_schema.json';

monaco.languages.yaml.yamlDefaults.setDiagnosticsOptions({
  validate: true,
  schemas: [{
    uri: 'https://chartml.org/chartml_schema.json',
    fileMatch: ['*.chartml.yaml'],
    schema: schema
  }]
});
```

### Programmatic Validation

**Node.js** (with Ajv):
```javascript
import Ajv from 'ajv';
import schema from './chartml_schema.json';
import yaml from 'js-yaml';

const ajv = new Ajv();
const validate = ajv.compile(schema);

const chartml = yaml.load(yourChartmlString);
const valid = validate(chartml);

if (!valid) {
  console.error(validate.errors);
}
```

**Python** (with jsonschema):
```python
import json
import yaml
from jsonschema import validate

with open('chartml_schema.json') as f:
    schema = json.load(f)

with open('your_chart.chartml.yaml') as f:
    chartml = yaml.safe_load(f)

validate(instance=chartml, schema=schema)
```

## Schema Features

The ChartML schema provides:

✅ **Type Safety** - Validates data types (string, number, boolean, array, object)
✅ **Enum Validation** - Ensures chart types, aggregations, and other enums are valid
✅ **Required Fields** - Enforces mandatory properties
✅ **Pattern Matching** - Validates formats (dates, colors, parameter references)
✅ **Nested Validation** - Validates complex nested structures
✅ **Autocomplete** - Powers intelligent code completion in editors

## Schema Structure

The schema defines the complete ChartML v1.0 specification:

```
ChartML Root
├── source (optional) - Reusable data sources
├── params (optional) - Interactive parameters
├── style (optional) - Global styling
├── config (optional) - Global configuration
└── [chart blocks] - One or more chart definitions
    ├── data (required) - Data source (inline, SQL, or reference)
    ├── params (optional) - Chart-level parameters
    ├── aggregate (optional) - Data aggregation pipeline
    ├── visualize (required) - Chart specification
    ├── style (optional) - Chart-level styling
    └── config (optional) - Chart-level configuration
```

## Validation Errors

The schema provides detailed error messages for common mistakes:

**Invalid chart type:**
```
Error: visualize.type must be one of:
  bar, line, area, pie, doughnut, table, metric, scatter
```

**Missing required field:**
```
Error: visualize.columns is required for bar charts
```

**Invalid parameter reference:**
```
Error: Parameter reference $params.foo is undefined
```

## Contributing

Found an issue with the schema? Have a suggestion for improvement?

- [Report an issue](https://github.com/chartml/chartml/issues)
- [Submit a pull request](https://github.com/chartml/chartml/pulls)

## License

The ChartML JSON Schema is released under the MIT License.
