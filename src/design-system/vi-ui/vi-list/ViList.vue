<script setup lang="ts">
// ViList — wrapper iNET ViUI quanh v-list. Danh sách (vd segment đã ghi).
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-list v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-list>
</template>
