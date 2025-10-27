FROM amazoncorretto:21-alpine

ENV TZ=America/Sao_Paulo

WORKDIR /app
EXPOSE 8080

COPY build/libs/server-java-all.jar /app/server.jar

ENTRYPOINT ["java", "-jar", "-Dfile.encoding=UTF-8", "/app/server.jar"]
