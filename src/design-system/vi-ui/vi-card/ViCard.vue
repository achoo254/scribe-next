<script setup lang="ts">
// ViCard — wrapper iNET ViUI quanh v-card. Container nội dung, surface theo theme.
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-card v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-card>
</template>
