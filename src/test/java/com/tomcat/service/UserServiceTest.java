package com.tomcat.service;

import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import org.hamcrest.Matchers;
import org.junit.Assert;
import org.junit.jupiter.api.Test;
import org.junit.runner.RunWith;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.context.junit4.SpringRunner;

import javax.annotation.Resource;

import static org.junit.jupiter.api.Assertions.*;

@SpringBootTest
@RunWith(SpringRunner.class)
class UserServiceTest {

    @Resource
    private UserService userService;

    @Test
    void registerOrLogin() {
        AuthenticationRequest request = new AuthenticationRequest("tom", "1234", "", "13823232232", "8888888");
        AuthenticationResponse authenticationResponse = userService.registerOrLogin(request);
        System.out.println("registerOrLogin!");
        System.out.println("AccessToken= "+ authenticationResponse.getAccessToken());
        Assert.assertThat(authenticationResponse, Matchers.notNullValue());
    }
}