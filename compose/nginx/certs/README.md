# TLS Certificates for FullStackHex Nginx

## Expected Certificate Files

Place your TLS certificates in this directory:

- `fullchain.pem` - The full certificate chain (including intermediate certificates)
- `privkey.pem` - The private key file (keep this secure!)

## Obtaining Certificates

### Option 1: Let's Encrypt (Recommended for production)

```bash
# Install certbot
sudo apt-get install certbot

# Obtain certificate (requires domain pointing to this server)
sudo certbot certonly --standalone -d yourdomain.com

# Copy certificates to this directory
sudo cp /etc/letsencrypt/live/yourdomain.com/fullchain.pem ./fullchain.pem
sudo cp /etc/letsencrypt/live/yourdomain.com/privkey.pem ./privkey.pem

# Set proper permissions
sudo chmod 644 fullchain.pem
sudo chmod 600 privkey.pem
```

### Option 2: Self-Signed (Development/Testing Only)

```bash
# Generate self-signed certificate
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout privkey.pem \
    -out fullchain.pem \
    -subj "/C=US/ST=State/L=City/O=Organization/CN=localhost"
```

## Security Notes

- Never commit private keys to version control
- `.gitignore` already ignores `compose/nginx/certs/*.pem`, `*.key`, `*.crt`, and `*.p12` — no manual update needed
- Use strong file permissions (600 for privkey.pem)
- Consider using Docker secrets or a vault for production
