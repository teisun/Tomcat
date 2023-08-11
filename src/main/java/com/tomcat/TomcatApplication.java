package com.tomcat;

import com.sun.xml.bind.v2.TODO;
import com.tomcat.config.DataSourceConfig;
import com.tomcat.config.SecurityConfig;
import com.tomcat.service.impl.UserServiceImpl;
import com.unfbx.chatgpt.OpenAiStreamClient;
import com.unfbx.chatgpt.function.KeyRandomStrategy;
import com.unfbx.chatgpt.interceptor.OpenAILogger;
import okhttp3.OkHttpClient;
import okhttp3.logging.HttpLoggingInterceptor;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.ApplicationContext;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.ComponentScan;
import org.springframework.context.annotation.Import;
import org.springframework.data.jpa.repository.config.EnableJpaRepositories;

import javax.annotation.PostConstruct;
import java.util.List;
import java.util.concurrent.TimeUnit;

@SpringBootApplication
//@ComponentScan(basePackages = {"com.tomcat.utils", "com.tomcat.service", "com.tomcat.domain", "com.tomcat.config"})
//@EnableJpaRepositories(basePackages = "com.tomcat.domain")
//@Import({DataSourceConfig.class})
public class TomcatApplication {

    @Value("${chatgpt.apiKey}")
    private List<String> apiKey;
    @Value("${chatgpt.apiHost}")
    private String apiHost;


    public static void main(String[] args) {
        System.out.println("Hello Tomcat!");
        SpringApplication.run(TomcatApplication.class, args);


    }

    //TODO
    // 1. 支持存储中文
    // 2. findByDeviceId改造
    // 3. 修改userId



}