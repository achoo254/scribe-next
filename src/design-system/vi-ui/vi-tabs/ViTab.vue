<script setup lang="ts">
// ViTab — wrapper iNET ViUI quanh v-tab. Một mục trong ViTabs (prop value).
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-tab v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-tab>
</template>
