console.log('Setup file loaded');

// This file has side effects but also exports
export const config = {
  apiUrl: 'https://api.example.com',
  timeout: 5000,
};

export function initialize() {
  console.log('Initializing app');
}
