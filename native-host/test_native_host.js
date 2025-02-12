const { spawn } = require('child_process');
const path = require('path');

// Create a test message
const message = {
    type: 'check_hover',
    data: {}  // Add empty data object to match Message struct
};

// Convert message to native messaging format
const messageBuffer = Buffer.from(JSON.stringify(message));
const lengthBuffer = Buffer.alloc(4);
lengthBuffer.writeUInt32LE(messageBuffer.length, 0);

// Spawn the native host
const nativeHost = spawn(
    // Get path to native host executable relative to this script
    path.resolve(__dirname, 'target', 'release', 'native-host.exe'),
    [],
    {
        env: {
            ...process.env,
            RUST_LOG: 'debug'
        }
    }
);

// Log output
nativeHost.stdout.on('data', (data) => {
    // First 4 bytes are length
    const length = data.readUInt32LE(0);
    const jsonStr = data.slice(4, 4 + length).toString();
    console.log('Received:', jsonStr);
});

nativeHost.stderr.on('data', (data) => {
    console.error('Native host stderr:', data.toString());
});

nativeHost.on('error', (err) => {
    console.error('Failed to start native host:', err);
});

nativeHost.on('close', (code) => {
    console.log('Native host exited with code:', code);
});

// Countdown and send message after delay
console.log('Waiting 5 seconds before sending message...');
let countdown = 5;
const timer = setInterval(() => {
    console.log(`${countdown} seconds remaining...`);
    countdown--;
    if (countdown === 0) {
        clearInterval(timer);
        console.log('Sending message now!');
        // Send the test message
        nativeHost.stdin.write(Buffer.concat([lengthBuffer, messageBuffer]));
    }
}, 1000);
