# Build stage
FROM node:20-alpine AS builder

WORKDIR /app

# Install deps
COPY package.json package-lock.json ./
RUN npm ci --verbose

# Build app
COPY . .
RUN npm run build

# Run stage
FROM nginx:1.27-alpine

# Serve static files and proxy API
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
