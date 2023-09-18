package com.tomcat.utils;

import org.springframework.beans.BeansException;
import org.springframework.context.ApplicationContext;
import org.springframework.context.ApplicationContextAware;
import org.springframework.stereotype.Component;

/**
 * 这里必须实现ApplicationContextAware接口，注入上下文容器
 */
@Component
public class SpringContextUtils implements ApplicationContextAware {

    private static ApplicationContext applicationContext = null;

    /**
     * 获取上下文对象
     *
     * @return
     */
    public static ApplicationContext getApplicationContext() {
        return applicationContext;
    }

    /**
     * 通过对象在容器中的名称获取对象
     *
     * @param beanName
     * @param <T>
     * @return
     */
    public static <T> T getBean(String beanName) {
        return (T) applicationContext.getBean(beanName);
    }

    /**
     * 通过class类型获取容器中的对象
     *
     * @param beanClass
     * @param <T>
     * @return
     */
    public static <T> T getBean(Class<T> beanClass) {
        return (T) applicationContext.getBean(beanClass);
    }

    /**
     * Spring容器启动后，会把 applicationContext 给自动注入进来，然后我们把 applicationContext 赋值到静态变量中，方便后续拿到容器对象
     */
    @Override
    public void setApplicationContext(ApplicationContext applicationContext) throws BeansException {
        SpringContextUtils.applicationContext = applicationContext;
    }

}

