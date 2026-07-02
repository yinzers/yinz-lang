---
name: "SCRATCH-stdlib-dates"
description: "First-class date handling. Intuitive, immutable, timezone-aware. No JavaScript Date nonsense."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Standard Library — Dates & Time

First-class date handling. Intuitive, immutable, timezone-aware. No JavaScript Date nonsense.

Design goals: similar feel to dayjs but built-in, fully typed, compiled.

---

## Creating Dates

```
let now = date.now()
let birthday = date.from(1990, 6, 15)                   // year, month, day — NOT zero-indexed
let withTime = date.from(2026, 5, 11, 14, 30, 0)        // year, month, day, hour, minute, second
let parsed = date.parse("2026-05-11")                    // ISO string
let parsed2 = date.parse("May 11, 2026")                 // natural language
let fromTimestamp = date.fromUnix(1715000000)
let fromMillis = date.fromMillis(1715000000000)
```

Key decision: months are NOT zero-indexed. May = 5, not 4.

---

## Manipulating — all methods return a NEW date (immutable)

```
let tomorrow = now.addDays(1)
let nextWeek = now.addWeeks(1)
let nextMonth = now.addMonths(1)
let nextYear = now.addYears(1)
let later = now.addHours(3)
let laterMore = now.addMinutes(45)

let yesterday = now.subtractDays(1)
let lastMonth = now.subtractMonths(1)

let startOfDay = now.startOfDay()
let endOfDay = now.endOfDay()
let startOfMonth = now.startOfMonth()
let endOfMonth = now.endOfMonth()
let startOfYear = now.startOfYear()
let endOfYear = now.endOfYear()
let startOfWeek = now.startOfWeek()         // Monday
```

---

## Comparing

```
if (deadline.isBefore(now)) { ... }
if (birthday.isAfter(now)) { ... }
if (date1.isSameDay(date2)) { ... }
if (date1.isSameMonth(date2)) { ... }
if (date1.isSameYear(date2)) { ... }
if (now.isBetween(startDate, endDate)) { ... }
if (now.isToday()) { ... }
if (now.isPast()) { ... }
if (now.isFuture()) { ... }

let daysBetween = deadline.daysSince(startDate)
let hoursBetween = endtime.hoursSince(startTime)
let minutesBetween = end.minutesSince(start)
let monthsBetween = end.monthsSince(start)
let yearsBetween = end.yearsSince(start)
```

---

## Reading Parts

```
now.year                    // 2026
now.month                   // 5 (May — NOT zero-indexed)
now.day                     // 11
now.hour                    // 14
now.minute                  // 30
now.second                  // 0
now.millisecond             // 0
now.dayOfWeek               // "Monday"
now.dayOfWeekNumber         // 1 (Monday=1, Sunday=7)
now.dayOfYear               // 131
now.weekOfYear              // 20
now.quarter                 // 2
now.isWeekend()             // false
now.isWeekday()             // true
now.isLeapYear()            // false
now.daysInMonth()           // 31
```

---

## Formatting

```
now.format("MMMM D, YYYY")              // "May 11, 2026"
now.format("MM/DD/YY")                  // "05/11/26"
now.format("YYYY-MM-DD")               // "2026-05-11"
now.format("h:mm A")                    // "2:30 PM"
now.format("HH:mm:ss")                 // "14:30:00"
now.format("dddd")                      // "Monday"
now.toISO()                              // "2026-05-11T14:30:00Z"
now.toUnix()
now.toMillis()
now.toRelative()                         // "3 hours ago" / "in 2 days"
```

---

## Time Zones

```
let eastern = now.inTimeZone("America/New_York")
let utc = now.toUTC()
let tokyo = now.inTimeZone("Asia/Tokyo")
let local = utcDate.toLocal()
now.timeZone
now.utcOffset
```

---

## Duration Type

```
let meeting = duration.hours(2).minutes(30)
let later = now.add(meeting)
let remaining = deadline.durationUntil(now)
print(remaining.humanize())              // "3 days, 4 hours"

duration.seconds(90).humanize()          // "1 minute, 30 seconds"
remaining.totalSeconds()
remaining.totalMinutes()
remaining.totalHours()
remaining.totalDays()
```

---

## Expansion Candidates

- Calendar system support (Islamic, Hebrew, etc.)
- Recurring event / schedule support (every Monday, every 2 weeks)
- Business day calculations (skip weekends/holidays)
- Holiday awareness by locale
- DateRange type (contains, overlaps, etc.)
- Cron-style scheduling expressions
- Astronomical calculations (sunrise, sunset, moon phase)
- Age calculation helpers
