<script setup lang="ts">
// ViTabs — wrapper iNET ViUI quanh v-tabs. Tab switcher (shell 3 tab OneScribe).
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-tabs v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-tabs>
</template>
