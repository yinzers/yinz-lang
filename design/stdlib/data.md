# Standard Library — Data Analysis & Processing

---

## CSV

```
let data = csv.parse(file.read("sales.csv"))
let typed = csv.parseAs<SalesRecord>(file.read("sales.csv"))
```

---

## JSON

```
let obj = json.parse(content)
let typed = json.parseAs<Config>(content)
let output = json.stringify(data)
let pretty = json.stringify(data, indent: 2)
```

---

## Data Operations on Collections

```
let records = csv.parseAs<SalesRecord>(file.read("data.csv"))
let q4 = records.filter(r => r.quarter == "Q4")
let sorted = q4.sort(r => r.revenue, desc)
let total = q4.map(r => r.revenue).sum()
let avg = q4.map(r => r.revenue).average()

let byRegion = records.groupBy(r => r.region)
```

Note: `.sum()` and `.average()` are collection methods available on `array<number>` / `fixed<number>`.

---

## Expansion Candidates

- YAML parsing
- TOML parsing
- XML parsing
- Parquet file support
- Data frame type (like pandas DataFrame)
- Pivot table operations
- SQL-like query syntax over in-memory data
- Excel file read/write
- Data validation and schema enforcement
- Streaming parsers for large files
