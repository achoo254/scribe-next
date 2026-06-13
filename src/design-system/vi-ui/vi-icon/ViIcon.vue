<script setup lang="ts">
// ViIcon — wrapper iNET ViUI quanh v-icon. Icon = Tabler (qua iconset tabler ở vuetify.ts).
// Dùng: <ViIcon icon="IconMicrophone" /> — tên là component @tabler/icons-vue.
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-icon v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-icon>
</template>
