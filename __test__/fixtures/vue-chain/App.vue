<template>
  <div>
    <Child />
  </div>
</template>

<script setup>
import { ChildComponent } from './Child.vue';

export const App = {
  name: 'App'
};

export const useApp = () => {
  return { component: ChildComponent };
};
</script>
