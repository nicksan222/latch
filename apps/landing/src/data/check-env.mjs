for (const name of ['DATABASE_URL', 'API_TOKEN']) {
  console.log(`${name} ${process.env[name] ? 'available to child process' : 'missing'}`);
}
