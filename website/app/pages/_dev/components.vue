<script setup lang="ts">
// Internal component gallery — noindex, never linked from public pages
useHead({ meta: [{ name: 'robots', content: 'noindex' }] })
</script>

<template>
  <YContainer class="py-16">
    <YHeading :level="1" class="mb-12">Component Gallery</YHeading>

    <!-- Layout -->
    <section class="mb-16">
      <YEyebrow class="mb-4">Layout</YEyebrow>
      <YHeading :level="3" class="mb-6">YContainer / YSection / YRow / YGrid / YDivider</YHeading>

      <div class="bg-bg-card border border-line rounded p-6 mb-4">
        <p class="text-ink-mute text-sm mb-2">YContainer wraps this whole page. YSection below:</p>
        <YSection class="bg-bg-raised px-4 rounded">
          <p class="text-ink">Content inside YSection</p>
        </YSection>
      </div>

      <div class="bg-bg-card border border-line rounded p-6 mb-4">
        <YRow class="mb-3">
          <span class="text-ink">YRow item 1</span>
          <span class="text-gold">YRow item 2</span>
          <span class="text-ink-mute">YRow item 3</span>
        </YRow>
        <YGrid cols="grid-cols-3" gap="gap-3">
          <div class="bg-bg-raised p-3 rounded text-ink text-sm">Grid 1</div>
          <div class="bg-bg-raised p-3 rounded text-ink text-sm">Grid 2</div>
          <div class="bg-bg-raised p-3 rounded text-ink text-sm">Grid 3</div>
        </YGrid>
      </div>

      <YDivider />
      <p class="text-ink-dim text-sm">YDivider above</p>
    </section>

    <!-- Typography -->
    <section class="mb-16">
      <YEyebrow class="mb-4">Typography</YEyebrow>
      <YHeading :level="3" class="mb-6">YEyebrow / YHeading</YHeading>

      <div class="bg-bg-card border border-line rounded p-6 space-y-4">
        <div>
          <YEyebrow>Eyebrow default (gold)</YEyebrow>
          <YEyebrow :mute="true">Eyebrow muted</YEyebrow>
        </div>
        <YHeading :level="1">Heading Level 1</YHeading>
        <YHeading :level="2">Heading Level 2</YHeading>
        <YHeading :level="3">Heading Level 3</YHeading>
        <YHeading :level="4">Heading Level 4</YHeading>
      </div>
    </section>

    <!-- Indicators -->
    <section class="mb-16">
      <YEyebrow class="mb-4">Indicators</YEyebrow>
      <YHeading :level="3" class="mb-6">YPill / YCallout</YHeading>

      <div class="bg-bg-card border border-line rounded p-6 space-y-4">
        <div class="flex flex-wrap gap-3">
          <YPill :dot="null">No dot</YPill>
          <YPill dot="">Gold dot (empty string → default gold)</YPill>
          <YPill :dot="'#5a8fa3'">River dot</YPill>
          <YPill :dot="'#c8442a'">Ember dot</YPill>
        </div>

        <YCallout variant="note" tag="Note">
          This is a <strong>note</strong> callout. Gold left border.
        </YCallout>
        <YCallout variant="info" tag="Info">
          This is an <strong>info</strong> callout. River teal left border.
        </YCallout>
        <YCallout variant="warn" tag="Warning">
          This is a <strong>warning</strong> callout. Ember red left border.
        </YCallout>
      </div>
    </section>
  </YContainer>
</template>
